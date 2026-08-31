import type { OpenApiSchemas } from "./generated/openapi.js";

export type {
  OpenApiSchema,
  OpenApiSchemaName,
  OpenApiSchemas,
} from "./generated/openapi.js";

type Schema<Name extends keyof OpenApiSchemas> = OpenApiSchemas[Name];

export type AuthRole = Schema<"AuthRole">;
export type CommandType = Schema<"CommandRequest">["commandType"];
export type EventCategory = Schema<"EventRecord">["category"];
export type EventSeverity = Schema<"EventRecord">["severity"];
export type EventOverflowPolicy = Schema<"EventSettings">["overflow_policy"];
export type RuntimeBackend = NonNullable<Schema<"EbpfRunRequest">["runtime_backend"]>;
export type RunnerAgentIsolation = Schema<"RunnerAgent">["isolation"];
export type RunnerAgentState = Schema<"RunnerAgent">["state"];
export type RunnerJobState = Schema<"RunnerJobView">["state"];

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export interface CyanrexClientOptions {
  fetch?: FetchLike;
  credentials?: RequestCredentials;
  headers?: Record<string, string>;
  csrfOrigin?: string;
  sessionCookie?: string;
}

export interface RequestOptions {
  signal?: AbortSignal;
}

export type ApiMessage = Schema<"ApiMessage">;
export type SystemInfo = Schema<"SystemInfo">;
export type HealthResponse = Schema<"HealthResponse">;

export interface OpenApiDocument {
  openapi: string;
  info: {
    title: string;
    version: string;
    description?: string;
  };
  paths: Record<string, Record<string, unknown>>;
  components?: Record<string, unknown>;
}

export type SessionResponse = Schema<"SessionResponse">;
export type LoginRequest = Schema<"LoginRequest">;
export type LoginResponse = Schema<"LoginResponse">;
export type TotpBootstrapRequest = Schema<"TotpBootstrapRequest">;
export type TotpBootstrapResponse = Schema<"TotpBootstrapResponse">;
export type RegisterRequest = Schema<"RegisterRequest">;
export type ChangePasswordRequest = Schema<"ChangePasswordRequest">;
export type DeleteAccountRequest = Schema<"DeleteAccountRequest">;

export type ModuleInfo = Schema<"ModuleInfo">;
export type HeaderModuleItem = Schema<"HeaderModuleItem">;
export type SelectedHeaderMetadata = Schema<"SelectedHeaderMetadata">;
export type CommandRequest = Schema<"CommandRequest">;
export type CommandResponse = Schema<"CommandResponse">;

export type EventRecord = Schema<"EventRecord">;

export interface EventQuery {
  category?: EventCategory;
  severity?: EventSeverity;
  limit?: number;
  since_minutes?: number;
  start?: string;
  end?: string;
}

export interface EventExportQuery extends Omit<EventQuery, "limit"> {
  format?: "json" | "csv";
}

export interface ApiDownload {
  blob: Blob;
  filename: string | null;
  contentType: string | null;
}

export type EventSettings = Schema<"EventSettings">;
export type UpdateEventSettingsResponse = Schema<"UpdateEventSettingsResponse">;
export type CompilerSettings = Schema<"CompilerSettings">;
export type UpdateCompilerSettingsResponse = Schema<"UpdateCompilerSettingsResponse">;
export type CompilerOperationMetrics = Schema<"CompilerOperationMetrics">;
export type PerformanceMetrics = Schema<"PerformanceMetrics">;

export type UserScript = Schema<"UserScript">;
export type SaveScriptResponse = Schema<"SaveScriptResponse">;

export type LabDefinition = Schema<"LabDefinition">;
export type LabProgress = Schema<"LabProgress">;
export type LabAttempt = Schema<"LabAttempt">;
export type StudentLearningOverview = Schema<"StudentLearningOverview">;
export type TeacherLearningOverview = Schema<"TeacherLearningOverview">;
export type TeacherStudentAttempts = Schema<"TeacherStudentAttempts">;

export type EbpfCompilerDiagnostic = Schema<"EbpfDiagnostic">;
export type EbpfCheckResponse = Schema<"EbpfCheckResponse">;
export type EbpfRunRequest = Schema<"EbpfRunRequest">;
export type EbpfRunResponse = Schema<"EbpfRunResponse">;
export type EbpfDebugInfo = Schema<"EbpfDebugInfo">;
export type EbpfCompletionItem = Schema<"EbpfCompletionResponse">["items"][number];
export type EbpfCompletionResponse = Schema<"EbpfCompletionResponse">;
export type EbpfTemplate = Schema<"EbpfTemplate">;
export type EbpfDetachResponse = Schema<"EbpfDetachResponse">;
export type EbpfAttachment = Schema<"EbpfAttachment">;
export type EbpfCheckBackend = Schema<"EbpfCheckBackendInventory">["agents"][number];
export type EbpfCheckBackendInventory = Schema<"EbpfCheckBackendInventory">;
export type EbpfRemoteCheckSubmitRequest = Schema<"EbpfRemoteCheckSubmitRequest">;
export type EbpfRemoteCheckResponse = Schema<"EbpfRemoteCheckResponse">;

export type RunnerStatus = Schema<"RunnerStatus">;
export type RunnerLease = Schema<"RunnerLease">;
export type RunnerOverview = Schema<"RunnerOverview">;
export type RunnerAgent = Schema<"RunnerAgent">;
export type RunnerAgentInventory = Schema<"RunnerAgentInventory">;
export type RunnerProbeRequest = Schema<"RunnerProbeRequest">;
export type RunnerCompileCheckRequest = Schema<"RunnerCompileCheckRequest">;
export type RunnerJob = Schema<"RunnerJobView">;
export type RunnerJobInventory = Schema<"RunnerJobInventory">;

export type EnvironmentReport = Schema<"EnvironmentReport">;
