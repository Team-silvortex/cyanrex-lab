export type AuthRole = "admin" | "teacher" | "student";
export type CommandType = "ListModules" | "StartModule" | "StopModule" | "RunExperiment";
export type EventCategory = "kernel" | "platform";
export type EventSeverity = "success" | "warning" | "error";
export type EventOverflowPolicy = "drop_oldest" | "drop_new";
export type RuntimeBackend = "bpftool" | "aya";
export type RunnerAgentIsolation =
  | "shared_kernel"
  | "container"
  | "virtual_machine"
  | "dedicated_host";
export type RunnerAgentState = "healthy" | "degraded" | "draining" | "offline";
export type RunnerJobState =
  | "queued"
  | "claimed"
  | "cancel_requested"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "expired";

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

export interface ApiMessage {
  ok: boolean;
  message: string;
}

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

export interface SessionResponse {
  authenticated: boolean;
  username: string | null;
  role: AuthRole | null;
  expires_at: string | null;
}

export interface LoginRequest {
  username: string;
  password: string;
  otp: string;
}

export interface LoginResponse extends ApiMessage {
  username: string | null;
  role: AuthRole | null;
  expires_at: string | null;
}

export interface TotpBootstrapRequest {
  username: string;
  password: string;
}

export interface TotpBootstrapResponse extends ApiMessage {
  issuer: string | null;
  account_name: string | null;
  secret: string | null;
  otpauth_uri: string | null;
}

export interface RegisterRequest {
  username: string;
  password: string;
}

export interface ChangePasswordRequest {
  current_password: string;
  new_password: string;
  otp: string;
}

export interface DeleteAccountRequest {
  password: string;
  otp: string;
}

export interface ModuleInfo {
  name: string;
  status: string;
}

export interface HeaderModuleItem {
  id: string;
  name: string;
  description: string;
  source_url: string;
  downloaded: boolean;
  selected: boolean;
  local_path: string;
}

export interface SelectedHeaderMetadata {
  id: string;
  include_hint: string;
  local_path: string;
  downloaded: boolean;
}

export interface CommandRequest {
  commandType: CommandType;
  moduleName?: string;
}

export interface CommandResponse extends ApiMessage {
  commandType: CommandType;
  modules?: ModuleInfo[];
  module?: ModuleInfo;
  nextPath?: string;
}

export interface EventRecord {
  username: string;
  timestamp: string;
  source: string;
  event_type: string;
  category: EventCategory;
  severity: EventSeverity;
  color: "green" | "yellow" | "red";
  payload: unknown;
}

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

export interface EventSettings {
  max_records: number;
  overflow_policy: EventOverflowPolicy;
}

export interface UpdateEventSettingsResponse extends ApiMessage {
  settings: EventSettings | null;
}

export interface CompilerSettings {
  resident: boolean;
  strategy: string;
}

export interface UpdateCompilerSettingsResponse extends ApiMessage {
  settings: CompilerSettings;
}

export interface CompilerOperationMetrics {
  total_requests: number;
  cache_hits: number;
  cache_misses: number;
  errors: number;
  rejected: number;
  in_flight: number;
  in_flight_peak: number;
  avg_duration_ms: number;
}

export interface PerformanceMetrics {
  check: CompilerOperationMetrics;
  completion: CompilerOperationMetrics;
}

export interface UserScript {
  id: string;
  username: string;
  title: string;
  script: string;
  created_at: string;
  updated_at: string;
}

export interface SaveScriptResponse extends ApiMessage {
  record: UserScript | null;
}

export interface LabDefinition {
  id: string;
  position: number;
  title: string;
  summary: string;
  doc_slug: string;
  template_id: string | null;
}

export interface LabProgress {
  lab: LabDefinition;
  status: "not_started" | "in_progress" | "completed";
  attempts: number;
  latest_stage: string | null;
  latest_feedback: string[];
  last_attempt_at: string | null;
  completed_at: string | null;
}

export interface LabAttempt {
  id: string;
  username: string;
  lab_id: string;
  template_id: string | null;
  source: string;
  source_sha256: string;
  run_success: boolean;
  stage: string;
  attach_expected: boolean;
  attach_verified: boolean;
  completed: boolean;
  feedback: string[];
  created_at: string;
}

export interface StudentLearningOverview {
  username: string;
  completed_labs: number;
  total_labs: number;
  total_attempts: number;
  last_activity_at: string | null;
  labs: LabProgress[];
}

export interface TeacherLearningOverview {
  generated_at: string;
  total_labs: number;
  active_students: number;
  students: StudentLearningOverview[];
}

export interface TeacherStudentAttempts {
  username: string;
  attempts: LabAttempt[];
}

export interface EbpfCompilerDiagnostic {
  line: number;
  column: number;
  end_column: number;
  severity: string;
  message: string;
}

export interface EbpfCheckResponse extends ApiMessage {
  diagnostics: EbpfCompilerDiagnostic[];
  stdout: string;
  stderr: string;
}

export interface EbpfRunRequest {
  code: string;
  template_id?: string;
  lab_id?: string;
  program_name?: string;
  runtime_backend?: RuntimeBackend;
  sampling_per_sec?: number;
  stream_seconds?: number;
  enable_kernel_stream?: boolean;
  debug_breakpoints?: number[];
}

export interface EbpfRunResponse {
  success: boolean;
  stage: string;
  message: string;
  compile_stdout: string;
  compile_stderr: string;
  load_stdout: string;
  load_stderr: string;
  pin_path: string | null;
  debug?: EbpfDebugInfo;
}

export interface EbpfDebugInfo {
  mode: string;
  session_id: string | null;
  requested_lines: number[];
  instrumented_lines: number[];
  rejected: Array<{ line: number; reason: string }>;
}

export interface EbpfCompletionItem {
  label: string;
  insert_text: string;
  detail: string;
  kind: string;
}

export interface EbpfCompletionResponse extends ApiMessage {
  items: EbpfCompletionItem[];
}

export interface EbpfTemplate {
  id: string;
  name: string;
  description: string;
  capability: string;
  category?: string;
  code: string;
}

export interface EbpfDetachResponse extends ApiMessage {
  detached: string[];
  clean: boolean;
  safety_notes: string[];
}

export interface EbpfAttachment {
  pin_path: string;
  source: string;
  program_name: string;
}

export interface EbpfCheckBackend {
  agent_id: string;
  isolation: RunnerAgentIsolation;
  state: RunnerAgentState;
  available_slots: number;
  max_concurrent: number;
}

export interface EbpfCheckBackendInventory {
  local_available: boolean;
  agents: EbpfCheckBackend[];
}

export interface EbpfRemoteCheckResponse {
  job_id: string;
  state: RunnerJobState;
  agent_id: string | null;
  message: string;
  result: EbpfCheckResponse | null;
}

export interface RunnerStatus {
  mode: string;
  isolation: string;
  instance_id: string;
  max_concurrent: number;
  max_per_user: number;
  active_total: number;
  active_for_current_user: number;
  available_slots: number;
  execution_timeout_seconds: number;
}

export interface RunnerLease {
  runner_id: string;
  username: string;
  runtime_backend: string;
  started_at: string;
  deadline: string;
}

export interface RunnerOverview {
  status: RunnerStatus;
  active_leases: RunnerLease[];
}

export interface RunnerAgent {
  agent_id: string;
  protocol_version: number;
  agent_version: string;
  isolation: RunnerAgentIsolation;
  state: RunnerAgentState;
  max_concurrent: number;
  active_jobs: number;
  available_slots: number;
  capabilities: string[];
  labels: Record<string, string>;
  kernel_release: string | null;
  message: string | null;
  registered_at: string;
  last_seen_at: string;
  expires_at: string;
}

export interface RunnerAgentInventory {
  generated_at: string;
  enabled: boolean;
  total_agents: number;
  online_agents: number;
  agents: RunnerAgent[];
}

export interface RunnerJob {
  job_id: string;
  kind: string;
  state: RunnerJobState;
  target_agent_id: string | null;
  assigned_agent_id: string | null;
  owner_username: string | null;
  message: string;
  source_bytes: number | null;
  program_name: string | null;
  timeout_seconds: number;
  result_message: string | null;
  output: string | null;
  created_at: string;
  claimed_at: string | null;
  deadline: string | null;
  completed_at: string | null;
}

export interface RunnerJobInventory {
  generated_at: string;
  total_jobs: number;
  jobs: RunnerJob[];
}

export interface EnvironmentReport {
  overall_ok: boolean;
  generated_at: string;
  runtime_mode: "native-linux" | "wsl2" | "docker";
  runtime_guidance: string;
  checks: Array<{ name: string; ok: boolean; detail: string }>;
}
