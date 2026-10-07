import { exact, invalid, jsonCopy, MAX_AGENT_CALLS, MAX_AGENT_RESULT_BYTES, parseArguments, record } from "./json.js";

export type AgentToolFormat = "openai_responses" | "openai_chat_completions" | "anthropic_messages"
  | "gemini_generate_content" | "mcp" | "custom";
export interface AgentCall { readonly id: string; readonly name: string; readonly arguments: Record<string, unknown> }
export interface AgentResult {
  readonly id: string;
  readonly name: string;
  readonly ok: boolean;
  readonly output?: unknown;
  readonly error?: { readonly code: string; readonly outcome: "not_attempted" | "unconfirmed" };
}
const formats = new Set(["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "mcp", "custom"]);

export function normalizeAgentCalls(format: AgentToolFormat, raw: unknown): AgentCall[] {
  if (!formats.has(format) || !Array.isArray(raw) || raw.length > MAX_AGENT_CALLS) invalid();
  const copied = jsonCopy(raw, MAX_AGENT_CALLS * (2 * 64 * 1024 + 1024)) as unknown[];
  const seen = new Set<string>();
  return copied.map(entry => {
    const call = record(jsonCopy(entry, 2 * 64 * 1024 + 1024));
    let id: unknown, name: unknown, args: unknown;
    switch (format) {
      case "openai_responses": {
        exact(call, ["type", "call_id", "name", "arguments"], ["id", "status"]);
        if (call.type !== "function_call" || call.status !== undefined && call.status !== "completed") invalid();
        if (call.id !== undefined && (typeof call.id !== "string" || call.id.length > 128)) invalid();
        id = call.call_id; name = call.name; args = parseArguments(call.arguments); break;
      }
      case "openai_chat_completions": {
        exact(call, ["id", "type", "function"]); if (call.type !== "function") invalid();
        const fn = exact(call.function, ["name", "arguments"]);
        id = call.id; name = fn.name; args = parseArguments(fn.arguments); break;
      }
      case "anthropic_messages":
        exact(call, ["type", "id", "name", "input"]); if (call.type !== "tool_use") invalid();
        id = call.id; name = call.name; args = call.input; break;
      case "gemini_generate_content": {
        exact(call, ["functionCall"]);
        const fn = exact(call.functionCall, ["id", "name", "args"]);
        id = fn.id; name = fn.name; args = fn.args; break;
      }
      case "mcp": {
        exact(call, ["id", "method", "params"], ["jsonrpc"]);
        if (call.method !== "tools/call" || call.jsonrpc !== undefined && call.jsonrpc !== "2.0") invalid();
        const params = exact(call.params, ["name"], ["arguments"]);
        id = call.id; name = params.name; args = params.arguments ?? {}; break;
      }
      case "custom":
        exact(call, ["id", "name", "arguments"]);
        id = call.id; name = call.name; args = call.arguments; break;
      default: return invalid();
    }
    if (typeof id !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/.test(id)
      || typeof name !== "string" || !/^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(name) || seen.has(id)) invalid();
    seen.add(id);
    return { id, name, arguments: record(jsonCopy(args)) };
  });
}

export function formatAgentResults(format: AgentToolFormat, results: readonly AgentResult[]): unknown[] {
  if (!formats.has(format) || !Array.isArray(results) || results.length > MAX_AGENT_CALLS) invalid();
  const copied = jsonCopy(results, MAX_AGENT_CALLS * (MAX_AGENT_RESULT_BYTES + 1024)) as AgentResult[];
  return copied.map(result => {
    const output = result.ok ? { ok: true, data: result.output } : { ok: false, error: result.error };
    const text = JSON.stringify(output);
    switch (format) {
      case "openai_responses": return { type: "function_call_output", call_id: result.id, output: text };
      case "openai_chat_completions": return { role: "tool", tool_call_id: result.id, content: text };
      case "anthropic_messages": return { type: "tool_result", tool_use_id: result.id, content: text, is_error: !result.ok };
      case "gemini_generate_content": return { functionResponse: { id: result.id, name: result.name, response: output } };
      case "mcp": return { content: [{ type: "text", text }], isError: !result.ok };
      case "custom": return { ...result };
      default: return invalid();
    }
  });
}
