import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { extractEngineAccess, extractEngineOperations } from "./openapi-contract.mjs";
import { array, ref, schemas } from "./openapi-schemas.mjs";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = path.join(projectRoot, "engine/openapi/openapi.json");

const requestSchemas = new Map(Object.entries({
  "POST /auth/delete": "DeleteAccountRequest",
  "POST /auth/login": "LoginRequest",
  "POST /auth/password/change": "ChangePasswordRequest",
  "POST /auth/register": "RegisterRequest",
  "POST /auth/totp/bootstrap": "TotpBootstrapRequest",
  "POST /command": "CommandRequest",
  "POST /ebpf/check": "EbpfCodeRequest",
  "POST /ebpf/check/remote": "EbpfRemoteCheckSubmitRequest",
  "POST /ebpf/check/remote/cancel": "EbpfRemoteCheckCancelRequest",
  "POST /ebpf/complete": "EbpfCompletionRequest",
  "POST /ebpf/detach": "EbpfDetachRequest",
  "POST /ebpf/run": "EbpfRunRequest",
  "POST /modules/c-headers/delete": "DownloadHeaderRequest",
  "POST /modules/c-headers/download": "DownloadHeaderRequest",
  "POST /modules/c-headers/select": "SelectHeaderRequest",
  "POST /modules/start": "ModuleControlRequest",
  "POST /modules/stop": "ModuleControlRequest",
  "POST /runner/agent/heartbeat": "RunnerAgentHeartbeatRequest",
  "POST /runner/agent/jobs/claim": "RunnerJobClaimRequest",
  "POST /runner/agent/jobs/result": "RunnerJobResultRequest",
  "POST /runner/agent/jobs/sync": "RunnerJobSyncRequest",
  "POST /runner/agent/register": "RunnerAgentRegisterRequest",
  "POST /runner/jobs/cancel": "RunnerJobCancelRequest",
  "POST /runner/jobs/compile-check": "RunnerCompileCheckRequest",
  "POST /runner/jobs/probe": "RunnerProbeRequest",
  "POST /scripts/delete": "DeleteScriptRequest",
  "POST /scripts/save": "SaveScriptRequest",
  "POST /settings/compiler": "UpdateCompilerSettingsRequest",
  "POST /settings/events": "EventSettings",
}));

const responseSchemas = new Map([
  ["GET /", ref("SystemInfo")],
  ["GET /auth/me", ref("SessionResponse")],
  ["GET /ebpf/attachments", objectWithArray("pin_paths", { type: "string" })],
  ["GET /ebpf/attachments/details", objectWithArray("attachments", ref("EbpfAttachment"))],
  ["GET /ebpf/check/backends", ref("EbpfCheckBackendInventory")],
  ["GET /ebpf/check/remote", ref("EbpfRemoteCheckResponse")],
  ["GET /ebpf/templates", array(ref("EbpfTemplate"))],
  ["GET /events", array(ref("EventRecord"))],
  ["GET /events/unread-count", simpleObject({ unread: { type: "integer", minimum: 0 } })],
  ["GET /health", ref("HealthResponse")],
  ["GET /helper/environment", ref("EnvironmentReport")],
  ["GET /learning/attempts", array(ref("LabAttempt"))],
  ["GET /learning/labs", array(ref("LabProgress"))],
  ["GET /learning/teacher/attempts", ref("TeacherStudentAttempts")],
  ["GET /learning/teacher/overview", ref("TeacherLearningOverview")],
  ["GET /modules", array(ref("ModuleInfo"))],
  ["GET /modules/c-headers/catalog", objectWithArray("headers", ref("HeaderModuleItem"))],
  ["GET /modules/c-headers/selected-metadata", objectWithArray("selected_headers", ref("SelectedHeaderMetadata"))],
  ["GET /runner/agents", ref("RunnerAgentInventory")],
  ["GET /runner/jobs", ref("RunnerJobInventory")],
  ["GET /runner/overview", ref("RunnerOverview")],
  ["GET /runner/status", ref("RunnerStatus")],
  ["GET /scripts", array(ref("UserScript"))],
  ["GET /settings/compiler", ref("CompilerSettings")],
  ["GET /settings/events", ref("EventSettings")],
  ["GET /settings/performance", ref("PerformanceMetrics")],
  ["POST /auth/delete", ref("ApiMessage")],
  ["POST /auth/login", ref("LoginResponse")],
  ["POST /auth/logout", ref("ApiMessage")],
  ["POST /auth/password/change", ref("ApiMessage")],
  ["POST /auth/register", ref("TotpBootstrapResponse")],
  ["POST /auth/totp/bootstrap", ref("TotpBootstrapResponse")],
  ["POST /command", ref("CommandResponse")],
  ["POST /ebpf/check", ref("EbpfCheckResponse")],
  ["POST /ebpf/check/remote", ref("EbpfRemoteCheckResponse")],
  ["POST /ebpf/check/remote/cancel", ref("EbpfRemoteCheckResponse")],
  ["POST /ebpf/complete", ref("EbpfCompletionResponse")],
  ["POST /ebpf/detach", ref("EbpfDetachResponse")],
  ["POST /ebpf/run", ref("EbpfRunResponse")],
  ["POST /events/delete", simpleObject({ ok: { type: "boolean" }, deleted: { type: "integer", minimum: 0 } })],
  ["POST /events/mark-read", simpleObject({ ok: { type: "boolean" } })],
  ["POST /modules/c-headers/delete", ref("ApiMessage")],
  ["POST /modules/c-headers/download", ref("ApiMessage")],
  ["POST /modules/c-headers/select", ref("ApiMessage")],
  ["POST /modules/start", ref("ModuleInfo")],
  ["POST /modules/stop", ref("ModuleInfo")],
  ["POST /runner/jobs/cancel", ref("RunnerJobView")],
  ["POST /runner/jobs/compile-check", ref("RunnerJobView")],
  ["POST /runner/jobs/probe", ref("RunnerJobView")],
  ["POST /runner/agent/heartbeat", ref("RunnerAgent")],
  ["POST /runner/agent/jobs/claim", ref("RunnerJobClaimResponse")],
  ["POST /runner/agent/jobs/result", ref("RunnerJobView")],
  ["POST /runner/agent/jobs/sync", ref("RunnerJobSyncResponse")],
  ["POST /runner/agent/register", ref("RunnerAgentRegistrationResponse")],
  ["POST /scripts/delete", ref("ApiMessage")],
  ["POST /scripts/save", ref("SaveScriptResponse")],
  ["POST /settings/compiler", ref("UpdateCompilerSettingsResponse")],
  ["POST /settings/events", ref("UpdateEventSettingsResponse")],
]);

const eventFilters = [
  query("category", { type: "string", enum: ["kernel", "platform"] }),
  query("severity", { type: "string", enum: ["success", "warning", "error"] }),
  query("since_minutes", { type: "integer", minimum: 0 }),
  query("start", { type: "string", format: "date-time" }),
  query("end", { type: "string", format: "date-time" }),
];
const queryParameters = new Map([
  ["GET /events", [...eventFilters, query("limit", { type: "integer", minimum: 1, maximum: 500 })]],
  ["GET /events/export", [...eventFilters, query("format", { type: "string", enum: ["json", "csv"] })]],
  ["POST /events/delete", eventFilters],
  ["GET /learning/teacher/attempts", [
    query("username", { type: "string" }, true),
    query("limit", { type: "integer", minimum: 1, maximum: 100 }),
  ]],
  ["GET /ebpf/check/remote", [query("job_id", { type: "string" }, true)]],
]);

const createdOperations = new Set([
  "POST /auth/register",
  "POST /runner/jobs/compile-check",
  "POST /runner/jobs/probe",
]);

async function buildDocument() {
  const [application, cargo] = await Promise.all([
    readFile(path.join(projectRoot, "engine/src/application.rs"), "utf8"),
    readFile(path.join(projectRoot, "engine/Cargo.toml"), "utf8"),
  ]);
  const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error("could not read Engine version");
  const paths = {};
  const accessByOperation = extractEngineAccess(application);
  for (const operation of extractEngineOperations(application)) {
    const [method, routePath] = operation.split(" ", 2);
    const access = accessByOperation[operation];
    if (!access) throw new Error(`could not determine Engine access tier for ${operation}`);
    paths[routePath] ??= {};
    paths[routePath][method.toLowerCase()] = buildOperation(operation, method, routePath, access);
  }
  return {
    openapi: "3.1.0",
    info: {
      title: "Cyanrex Lab Engine API",
      version,
      description: "Machine-readable contract for the Cyanrex teaching control and execution plane.",
    },
    servers: [{ url: "/", description: "Current Engine origin" }],
    tags: [...new Set(Object.values(paths).flatMap((item) => Object.values(item).flatMap((op) => op.tags)))]
      .sort()
      .map((name) => ({ name })),
    paths,
    components: {
      securitySchemes: securitySchemes(),
      schemas,
    },
  };
}

function buildOperation(operation, method, routePath, access) {
  const result = {
    operationId: operationId(method, routePath),
    summary: `${method} ${routePath}`,
    tags: [tagFor(routePath)],
    security: securityFor(access),
    responses: responsesFor(operation),
    "x-cyanrex-access": access,
  };
  const roles = rolesFor(access);
  if (roles) result["x-cyanrex-roles"] = roles;
  if (method === "POST" && access !== "public" && !access.startsWith("runner-agent")) {
    result["x-cyanrex-csrf"] = "Origin or Referer must match the configured frontend origin";
  }
  const parameters = queryParameters.get(operation);
  if (parameters) result.parameters = parameters;
  const requestSchema = requestSchemas.get(operation);
  if (requestSchema) {
    result.requestBody = {
      required: true,
      content: { "application/json": { schema: ref(requestSchema) } },
    };
  }
  return result;
}

function responsesFor(operation) {
  if (operation === "GET /ws/events") {
    return { 101: { description: "WebSocket protocol upgrade" } };
  }
  if (operation === "GET /events/export") {
    return {
      200: {
        description: "Event export download",
        content: {
          "application/json": { schema: array(ref("EventRecord")) },
          "text/csv": { schema: { type: "string" } },
        },
      },
      default: errorResponse(),
    };
  }
  const successCode = operation === "POST /ebpf/check/remote" ? 202
    : createdOperations.has(operation) ? 201
      : 200;
  const schema = operation === "GET /openapi.json"
    ? { type: "object", additionalProperties: true }
    : responseSchemas.get(operation) ?? { type: "object", additionalProperties: true };
  return {
    [successCode]: {
      description: "Successful response",
      content: { "application/json": { schema } },
    },
    default: errorResponse(),
  };
}

function securityFor(access) {
  if (access === "public") return [];
  if (access === "optional-session-csrf") return [{}, { cookieAuth: [] }];
  if (access === "runner-agent-bootstrap") return [{ runnerBootstrapBearer: [] }];
  if (access === "runner-agent-signed") {
    return [{ agentIdHeader: [], agentTimestampHeader: [], agentNonceHeader: [], agentSignatureHeader: [] }];
  }
  return [{ cookieAuth: [] }];
}

function rolesFor(access) {
  if (access === "admin") return ["admin"];
  if (access === "staff") return ["admin", "teacher"];
  if (access === "authenticated") return ["admin", "teacher", "student"];
  return null;
}

function tagFor(routePath) {
  if (routePath === "/" || routePath === "/health" || routePath === "/openapi.json") return "System";
  if (routePath.startsWith("/auth")) return "Authentication";
  if (routePath.startsWith("/ebpf")) return "eBPF";
  if (routePath.startsWith("/events") || routePath.startsWith("/ws/events")) return "Events";
  if (routePath.startsWith("/learning")) return "Learning";
  if (routePath.startsWith("/modules/c-headers")) return "C Headers";
  if (routePath.startsWith("/modules")) return "Modules";
  if (routePath.startsWith("/runner/agent")) return "Runner Agent";
  if (routePath.startsWith("/runner")) return "Runner";
  if (routePath.startsWith("/scripts")) return "Scripts";
  if (routePath.startsWith("/settings")) return "Settings";
  if (routePath.startsWith("/helper")) return "Environment";
  if (routePath.startsWith("/command")) return "Command";
  return "Other";
}

function operationId(method, routePath) {
  const suffix = routePath === "/" ? "Root" : routePath
    .split(/[^a-zA-Z0-9]+/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");
  return method.toLowerCase() + suffix;
}

function securitySchemes() {
  return {
    cookieAuth: { type: "apiKey", in: "cookie", name: "cyanrex_session" },
    runnerBootstrapBearer: { type: "http", scheme: "bearer" },
    agentIdHeader: { type: "apiKey", in: "header", name: "x-cyanrex-agent-id" },
    agentTimestampHeader: { type: "apiKey", in: "header", name: "x-cyanrex-agent-timestamp" },
    agentNonceHeader: { type: "apiKey", in: "header", name: "x-cyanrex-agent-nonce" },
    agentSignatureHeader: { type: "apiKey", in: "header", name: "x-cyanrex-agent-signature" },
  };
}

function query(name, schema, required = false) {
  return { name, in: "query", required, schema };
}

function simpleObject(properties) {
  return { type: "object", properties, required: Object.keys(properties), additionalProperties: false };
}

function objectWithArray(property, items) {
  return simpleObject({ [property]: array(items) });
}

function errorResponse() {
  return {
    description: "Error response",
    content: { "application/json": { schema: ref("ApiMessage") } },
  };
}

const document = await buildDocument();
const serialized = `${JSON.stringify(document, null, 2)}\n`;
if (process.argv.includes("--check")) {
  const existing = await readFile(outputPath, "utf8").catch(() => "");
  if (existing !== serialized) {
    throw new Error("engine/openapi/openapi.json is stale; run node scripts/generate-openapi.mjs");
  }
  console.log(`Generated OpenAPI document is current (${Object.keys(document.paths).length} paths).`);
} else {
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(`Wrote ${outputPath} with ${Object.keys(document.paths).length} paths.`);
}
