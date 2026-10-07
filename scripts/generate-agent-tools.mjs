import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { collectSdkOperations } from "./generate-sdk-operations.mjs";

// Deliberately reviewed, not automatically extended by new OpenAPI operations.
const policy = {
  getHealth: ["GET", "/health", "public", "Read Engine health."],
  getRoot: ["GET", "/", "public", "Read Engine information."],
  getHelperEnvironment: ["GET", "/helper/environment", "authenticated", "Read the current environment report."],
  getRunnerStatus: ["GET", "/runner/status", "authenticated", "Read the current user's Runner capacity."],
  getScripts: ["GET", "/scripts", "authenticated", "Read the current user's saved scripts."],
  getLearningLabs: ["GET", "/learning/labs", "authenticated", "Read the current user's lab progress."],
  getLearningAttempts: ["GET", "/learning/attempts", "authenticated", "Read the current user's previous submissions."],
  getLearningAttempt: ["GET", "/learning/attempt", "authenticated", "Read one current-user submission without executing it."],
  getEbpfTemplates: ["GET", "/ebpf/templates", "authenticated", "Read built-in source templates."],
  getEbpfAttachments: ["GET", "/ebpf/attachments", "authenticated", "Read current-user attachment paths without changing the kernel."],
  getEbpfAttachmentsDetails: ["GET", "/ebpf/attachments/details", "authenticated", "Read current-user attachment metadata."],
  getEbpfCheckBackends: ["GET", "/ebpf/check/backends", "authenticated", "Read eligible compiler backends without submitting work."],
  getEbpfCheckRemote: ["GET", "/ebpf/check/remote", "authenticated", "Read one current-user remote compilation result."],
  getEvents: ["GET", "/events", "authenticated", "Read the current user's retained event history."],
  getEventsUnreadCount: ["GET", "/events/unread-count", "authenticated", "Read the current user's unread event count."],
  getModules: ["GET", "/modules", "staff", "Read the teacher-visible module catalogue without starting modules."],
  getModulesCHeadersCatalog: ["GET", "/modules/c-headers/catalog", "staff", "Read the teacher-visible header catalogue."],
  getModulesCHeadersSelectedMetadata: ["GET", "/modules/c-headers/selected-metadata", "authenticated", "Read selected header metadata without changing selection."],
  getRunnerAgents: ["GET", "/runner/agents", "admin", "Read teacher-visible Runner computing nodes, not AI participants."],
  getRunnerJobs: ["GET", "/runner/jobs", "admin", "Read teacher-visible Runner jobs without submitting or cancelling."],
  getRunnerOverview: ["GET", "/runner/overview", "admin", "Read the teacher-visible Runner overview."],
  getLearningTeacherAttempts: ["GET", "/learning/teacher/attempts", "staff", "Read teacher-visible student submissions."],
  getLearningTeacherOverview: ["GET", "/learning/teacher/overview", "staff", "Read the teacher-visible classroom overview."],
  postScriptsSave: ["POST", "/scripts/save", "authenticated", "Save a current-user script only after trusted host approval."],
  postScriptsDelete: ["POST", "/scripts/delete", "authenticated", "Delete a current-user script only after trusted host approval."],
  postEbpfCheck: ["POST", "/ebpf/check", "authenticated", "Request compiler diagnostics after trusted host approval; never load code."],
  postEbpfComplete: ["POST", "/ebpf/complete", "authenticated", "Request code completion after trusted host approval; never load code."],
  postLearningTeacherFeedback: ["POST", "/learning/teacher/feedback", "staff", "Save revision-fenced teacher feedback only after trusted host approval."],
};

export function collectAgentTools(document) {
  const entries = [];
  for (const operation of collectSdkOperations(document)) {
    const selected = policy[operation.operationId];
    if (!selected) continue;
    const [method, route, access, description] = selected;
    if (operation.method !== method || operation.path !== route || operation.access !== access
      || operation.transport !== "json") throw new Error(`agent tool policy drift: ${operation.operationId}`);
    entries.push([operation.operationId, {
      operationName: operation.operationId, description, mutating: method !== "GET",
      inputSchema: expandSchema(operation.inputSchema, document),
    }]);
  }
  return Object.fromEntries(entries);
}

function expandSchema(schema, document, references = [], depth = 0) {
  if (depth > 32 || !schema || typeof schema !== "object" || Array.isArray(schema)) {
    throw new Error("invalid or excessive agent input schema");
  }
  if (schema.$ref) {
    const reference = schema.$ref;
    if (typeof reference !== "string" || !/^#\/components\/schemas\/[A-Za-z0-9_]+$/.test(reference)
      || Object.keys(schema).length !== 1) throw new Error("agent tools require a sole local schema reference");
    if (references.includes(reference)) throw new Error("recursive schema is not an agent input");
    const target = document.components?.schemas?.[reference.split("/").at(-1)];
    if (!target) throw new Error("missing local schema reference");
    return expandSchema(target, document, [...references, reference], depth + 1);
  }
  const result = { ...schema };
  for (const keyword of ["anyOf", "oneOf", "allOf"]) {
    if (schema[keyword]) result[keyword] = schema[keyword].map(item => expandSchema(item, document, references, depth + 1));
  }
  if (schema.properties) result.properties = Object.fromEntries(Object.entries(schema.properties)
    .map(([name, value]) => [name, expandSchema(value, document, references, depth + 1)]));
  if (schema.items) result.items = expandSchema(schema.items, document, references, depth + 1);
  if (schema.additionalProperties && typeof schema.additionalProperties === "object") {
    result.additionalProperties = expandSchema(schema.additionalProperties, document, references, depth + 1);
  }
  return result;
}

export function renderAgentTools(document) {
  const tools = collectAgentTools(document);
  return [
    "// Generated by scripts/generate-agent-tools.mjs; do not edit directly.",
    "// Explicit tool suitability policy is not server authorization or an AI delegation grant.",
    'import type { OpenApiOperationName } from "./operations.js";',
    "",
    "export const agentToolCatalog = {",
    ...Object.entries(tools).map(([name, tool]) => `  ${JSON.stringify(name)}: ${JSON.stringify(tool)},`),
    "} as const satisfies Record<string, {",
    "  operationName: OpenApiOperationName; description: string; mutating: boolean;",
    "  inputSchema: Readonly<Record<string, unknown>>;",
    "}>;",
    "",
    "export type AgentToolOperationName = keyof typeof agentToolCatalog;",
    "",
  ].join("\n");
}

async function main() {
  if (process.argv.slice(2).some(argument => argument !== "--check")) throw new Error("Usage: generate-agent-tools.mjs [--check]");
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const document = JSON.parse(await readFile(path.join(root, "engine/openapi/openapi.json"), "utf8"));
  const output = path.join(root, "sdk-js/src/generated/agent-tools.ts");
  const content = renderAgentTools(document);
  if (process.argv.includes("--check")) {
    if (await readFile(output, "utf8").catch(() => "") !== content) throw new Error("AI Agent tool catalogue is stale; regenerate explicitly");
    console.log(`AI Agent tool catalogue is synchronized (${Object.keys(collectAgentTools(document)).length} reviewed tools).`);
  } else {
    await writeFile(output, content);
    console.log("Generated reviewed AI Agent tool catalogue.");
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
