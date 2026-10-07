import type { CyanrexClient } from "../index.js";
import type { OpenApiOperationName } from "../generated/operations.js";
import { agentToolCatalog, type AgentToolOperationName } from "../generated/agent-tools.js";
import { normalizeAgentCalls, type AgentCall, type AgentResult, type AgentToolFormat } from "./calls.js";
import { freezeJson, invalid, jsonCopy, MAX_AGENT_RESULT_BYTES } from "./json.js";
import { validateToolArguments } from "./schema.js";

export type { AgentToolOperationName } from "../generated/agent-tools.js";
export interface AgentMutationApproval {
  readonly call: AgentCall;
  readonly operationName: AgentToolOperationName;
}
export interface AgentBridgeOptions {
  readonly operations: readonly AgentToolOperationName[];
  readonly allowMutations?: boolean;
  readonly approveMutation?: (request: AgentMutationApproval) => boolean | Promise<boolean>;
}
interface Tool {
  operationName: AgentToolOperationName;
  description: string;
  inputSchema: Record<string, unknown>;
  mutating: boolean;
}

/** A caller-owned, bounded dispatch context; not a durable receipt or AI identity. */
export function createAgentBridge(client: Pick<CyanrexClient, "operation">, options: AgentBridgeOptions) {
  if (!options || !Array.isArray(options.operations) || options.operations.length > 32
    || new Set(options.operations).size !== options.operations.length) invalid();
  const approve = options.approveMutation;
  const selected = new Map<string, Tool>();
  for (const name of options.operations as readonly AgentToolOperationName[]) {
    if (!Object.hasOwn(agentToolCatalog, name)) invalid();
    const tool = freezeJson(jsonCopy(agentToolCatalog[name])) as Tool;
    if (tool.mutating && (options.allowMutations !== true || typeof approve !== "function")) {
      throw new TypeError("Mutating agent tools require explicit permission and a trusted approval callback");
    }
    selected.set(name, tool);
  }
  const invoke = client.operation.bind(client) as (
    name: OpenApiOperationName, input: unknown, options?: { signal?: AbortSignal },
  ) => Promise<unknown>;
  const used = new Set<string>();
  let busy = false;

  function tools(format: AgentToolFormat): unknown[] {
    const definitions = [...selected.values()].map(tool => ({
      name: tool.operationName, description: tool.description, inputSchema: jsonCopy(tool.inputSchema),
    }));
    switch (format) {
      case "openai_responses": return definitions.map(({ name, description, inputSchema }) => ({
        type: "function", name, description, parameters: inputSchema, strict: false,
      }));
      case "openai_chat_completions": return definitions.map(({ name, description, inputSchema }) => ({
        type: "function", function: { name, description, parameters: inputSchema, strict: false },
      }));
      case "anthropic_messages": return definitions.map(({ name, description, inputSchema }) => ({ name, description, input_schema: inputSchema }));
      case "gemini_generate_content": return [{ functionDeclarations: definitions.map(({ name, description, inputSchema }) => ({
        name, description, parametersJsonSchema: inputSchema,
      })) }];
      case "mcp": case "custom": return definitions;
      default: return invalid();
    }
  }

  async function executeCalls(format: AgentToolFormat, raw: unknown, request: { signal?: AbortSignal } = {}): Promise<AgentResult[]> {
    if (busy) throw new TypeError("Agent bridge already has a pending batch");
    const calls = normalizeAgentCalls(format, raw);
    if (used.size + calls.length > 1024) throw new TypeError("Agent bridge call budget exhausted");
    // Validate the whole batch before any approval callback or network dispatch.
    for (const call of calls) {
      const tool = selected.get(call.name);
      if (!tool || used.has(call.id)) invalid();
      validateToolArguments(tool.inputSchema, call.arguments);
      freezeJson(call);
    }
    calls.forEach(call => used.add(call.id));
    busy = true;
    const results: AgentResult[] = [];
    try {
      for (const call of calls) {
        const tool = selected.get(call.name)!;
        const error = (code: string, outcome: "not_attempted" | "unconfirmed"): AgentResult => ({
          id: call.id, name: call.name, ok: false, error: { code, outcome },
        });
        if (request.signal?.aborted) { results.push(error("cancelled", "not_attempted")); continue; }
        if (tool.mutating) {
          let approved = false;
          try { approved = await approve!(Object.freeze({ call, operationName: tool.operationName })) === true; }
          catch { /* Trusted callback errors are not sent to the model. */ }
          if (!approved) { results.push(error("approval_denied", "not_attempted")); continue; }
          if (request.signal?.aborted) { results.push(error("cancelled", "not_attempted")); continue; }
        }
        try {
          const output = await invoke(tool.operationName, call.arguments, request);
          request.signal?.throwIfAborted();
          results.push({ id: call.id, name: call.name, ok: true, output: jsonCopy(output ?? null, MAX_AGENT_RESULT_BYTES) });
        } catch {
          // No raw error/URL/header/body disclosure and no automatic retry after dispatch.
          results.push(error("operation_failed", "unconfirmed"));
        }
      }
      return results;
    } finally { busy = false; }
  }
  return Object.freeze({ tools, executeCalls });
}
