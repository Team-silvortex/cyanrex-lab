import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { extractEngineAccess, extractEngineOperations } from "./openapi-contract.mjs";
import { array, ref, schemas } from "./openapi-schemas.mjs";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputPath = path.join(projectRoot, "engine/openapi/openapi.json");

const requestSchemas = new Map(Object.entries({
  "POST /classroom/invitations": "ClassroomInviteRequest",
  "POST /classroom/invitations/revoke": "ClassroomRevokeRequest",
  "POST /classroom/join": "ClassroomJoinRequest",
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
  "POST /learning/teacher/feedback": "SaveTeacherFeedbackRequest",
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
  ["GET /.well-known/cyanrex-classroom", ref("ClassroomDiscovery")],
  ["GET /classroom/invitations", ref("ClassroomInvitations")],
  ["POST /classroom/invitations", ref("ClassroomInvitation")],
  ["POST /classroom/invitations/revoke", ref("ApiMessage")],
  ["POST /classroom/join", ref("TotpBootstrapResponse")],
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
  ["GET /learning/attempt", ref("LabAttempt")],
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
  ["POST /learning/teacher/feedback", ref("LabTeacherFeedback")],
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
  ["GET /learning/attempt", [query("attempt_id", { type: "string", format: "uuid" }, true)]],
  ["GET /learning/teacher/attempts", [
    query("username", { type: "string" }, true),
    query("limit", { type: "integer", minimum: 1, maximum: 100 }),
  ]],
  ["GET /ebpf/check/remote", [query("job_id", { type: "string" }, true)]],
]);

const authMutationOperations = new Set(["POST /auth/logout", "POST /auth/password/change", "POST /auth/delete"]);
const learningReadOperations = new Set([
  "GET /learning/attempts", "GET /learning/labs",
  "GET /learning/teacher/attempts", "GET /learning/teacher/overview",
]);
const privateReadHeaders = {
  "Cache-Control": { description: "Do not cache learning records or storage-failure responses.", schema: { type: "string", const: "no-store" } },
};

const createdOperations = new Set([
  "POST /classroom/invitations",
  "POST /classroom/join",
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
  if (access === "admin") {
    result.description = "Deployment management is teacher authority. The legacy admin access-tier label and role remain compatible aliases, not a separate higher-privilege identity. Requires a teacher session and the existing CSRF policy for writes.";
  }
  if (operation === "GET /learning/attempt") {
    result.description = "Read one previous submission belonging to the authenticated session owner, including source and current teacher feedback. No username override or write occurs. Missing/other-owner records return 404; invalid IDs return 400; storage errors return 500. Responses use Cache-Control: no-store.";
  }
  if (learningReadOperations.has(operation)) {
    result.description = "Read learning records/projections without writing or executing a lab. A selected PostgreSQL schema/query failure returns 500 with no silent local fallback for this read. Invalid or unreadable local snapshots also return 500, not a successful empty history, zero progress or empty classroom. Missing local files remain valid empty state. Local load failures remain retryable after repair. Successful reads and storage failures use Cache-Control: no-store; errors omit stored values and paths.";
  }
  if (["POST /ebpf/check/remote", "GET /ebpf/check/remote", "POST /ebpf/check/remote/cancel"].includes(operation)) {
    result.description = "Owner-bound remote compile-only checks have a 35-second queue wait limit from submission. An unclaimed check becomes expired on the next queue interaction, releasing its source and per-user active quota. The claimed execution deadline is unchanged and begins at claim; this does not expire staff-managed unowned jobs or kill a running compiler. Queued cancellation is immediate; claimed cancellation requires acknowledgement or lease expiry. Missing/other-owner jobs return 404. No remote loading or silent local fallback is added.";
  }
  if (operation === "POST /runner/agent/jobs/claim") {
    result.description = "Signed claim requires a healthy Agent with a nonzero free-slot report. Total usable capacity is active_jobs + available_slots, already bounded by the registered maximum. Outstanding claimed and cancel-requested leases count once against that total; reserved capacity is not implicitly re-enabled. The claim response is no-store and includes the source and per-claim lease only for the assigned Agent.";
  }
  if (operation === "POST /runner/agent/jobs/sync") {
    result.description = "Signed lease synchronization reports cancellations and lost_job_ids. The bundled Agent discards its job before execution or result submission if that job is listed as lost, even if a cancellation instruction also names it. This rejects the stale job without stopping later polling; it is not continuous mid-execution lease monitoring.";
  }
  if (authMutationOperations.has(operation)) {
    result.description = "Configured PostgreSQL authentication must confirm the durable mutation before reporting success; no silent memory-only change or revocation on storage failure. Unconfirmed persistence returns 503 without clearing the session cookie; restore storage and verify state before an explicit retry. A lost commit/response acknowledgement can be ambiguous. Account deletion and its sessions share one transaction. Password/account writes bind the verified credentials and normalized identity; zero affected rows are rejected. Intentionally memory-only instances retain volatile behavior. After a read failure permanently disables auth persistence, these mutations remain blocked until storage is restored and Engine restarted.";
  }
  if (operation === "POST /auth/login") {
    result.description = "A login admitted against active PostgreSQL must confirm exactly one session insertion and recheck the verified credentials in the same transaction before publishing a session cookie. Concurrent account deletion/recreation or password changes cannot issue a stale login. Unconfirmed storage writes return 503 without issuing a cookie or switching this write to memory-only success. Intentionally volatile login remains available after previously established read/registration fallback; this is not distributed revocation.";
  }
  if (operation === "POST /events/delete") {
    result.description = "Delete only events matching all supplied filters for the authenticated session owner. No filters explicitly means delete all of that owner's events; since_minutes=0 adds no time restriction. Invalid/empty filter values, unknown query keys, negative or overflowing time windows, and reversed effective time ranges return 400 without deleting events. Legacy username and format query keys are ignored, never identity overrides. Surviving records retain their unread state.";
  }
  if (routePath.startsWith("/classroom/") || routePath === "/.well-known/cyanrex-classroom") {
    result.description = `${result.description ?? ""} Opt-in classroom onboarding; all responses are no-store. Discovery metadata is not proof of teacher identity. Use independently confirmed HTTPS origins (or trusted loopback SSH access). Invitations are student-name-bound, single-use, valid for 10 minutes and lost on restart. Join requires an invitation plus explicit classroom ID, compatible protocol and required capabilities, not a matching product patch. No automatic login, role promotion, Agent registration or eBPF execution occurs. Revoking an invitation does not revoke existing accounts or sessions.`.trim();
  }
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
    return { 101: { description: "Cookie-authenticated WebSocket; requires an allowed Origin/Referer (same CSRF policy as state changes). Text frames remain raw EventRecord JSON. Broadcast lag closes with code 1013 and reason 'event stream lagged; reload /events'. Sends time out after 5s (the peer may observe abnormal closure); close sends are bounded to 1s. Reconnect with backoff, then reload retained recent /events history. No durable cursor, exactly-once delivery, or complete gap recovery is guaranteed." } };
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
      ...(learningReadOperations.has(operation) ? { headers: privateReadHeaders } : {}),
    },
    default: errorResponse(),
    ...(learningReadOperations.has(operation) ? {
      500: { ...errorResponse(), description: "Learning storage read failed; not empty success. No storage details are exposed and no learning records are changed.", headers: privateReadHeaders },
    } : {}),
    ...(authMutationOperations.has(operation) ? {
      503: { ...errorResponse(), description: "Authentication storage unavailable; mutation completion is unconfirmed. Session cookie is retained for verification and an explicit retry." },
    } : {}),
    ...(operation === "POST /auth/login" ? {
      503: { ...errorResponse(), description: "Authentication storage unavailable; session insertion or commit is unconfirmed. No new session cookie is issued." },
    } : {}),
    ...(operation === "POST /events/delete" ? {
      400: {
        description: "Invalid deletion filter/query or time range; no events are deleted. Query extraction errors can be plain text.",
        content: { ...errorResponse().content, "text/plain": { schema: { type: "string" } } },
      },
    } : {}),
    ...(operation === "POST /classroom/join" ? {
      409: { ...errorResponse(), description: "Wrong classroom identity or account exists; inspect message before retrying." },
      426: { ...errorResponse(), description: "Incompatible join protocol/required capabilities; invitation not consumed. No automatic downgrade." },
    } : {}),
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
  if (access === "admin") return ["admin", "teacher"];
  if (access === "staff") return ["admin", "teacher"];
  if (access === "authenticated") return ["admin", "teacher", "student"];
  return null;
}

function tagFor(routePath) {
  if (routePath.startsWith("/classroom/") || routePath === "/.well-known/cyanrex-classroom") return "Classroom Connection";
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
