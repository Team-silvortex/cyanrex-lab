import type {
  ApiDownload,
  ApiMessage,
  ChangePasswordRequest,
  CommandRequest,
  CommandResponse,
  CompilerSettings,
  CyanrexClientOptions,
  DeleteAccountRequest,
  EbpfAttachment,
  EbpfCheckBackendInventory,
  EbpfCheckResponse,
  EbpfCompletionResponse,
  EbpfDetachResponse,
  EbpfRemoteCheckSubmitRequest,
  EbpfRemoteCheckResponse,
  EbpfRunRequest,
  EbpfRunResponse,
  EbpfTemplate,
  EnvironmentReport,
  EventExportQuery,
  EventQuery,
  EventRecord,
  EventSettings,
  FetchLike,
  HeaderModuleItem,
  HealthResponse,
  LabAttempt,
  LabProgress,
  LoginRequest,
  LoginResponse,
  ModuleInfo,
  OpenApiDocument,
  PerformanceMetrics,
  RegisterRequest,
  RequestOptions,
  RunnerAgentInventory,
  RunnerCompileCheckRequest,
  RunnerJob,
  RunnerJobInventory,
  RunnerOverview,
  RunnerProbeRequest,
  RunnerStatus,
  SaveScriptResponse,
  SelectedHeaderMetadata,
  SessionResponse,
  SystemInfo,
  TeacherLearningOverview,
  TeacherStudentAttempts,
  TotpBootstrapRequest,
  TotpBootstrapResponse,
  UpdateCompilerSettingsResponse,
  UpdateEventSettingsResponse,
  UserScript,
} from "./types.js";

export type * from "./types.js";

type Query = object;

export class CyanrexApiError extends Error {
  readonly status: number;
  readonly details: unknown;
  readonly method: string;
  readonly url: string;

  constructor(input: {
    message: string;
    status: number;
    details: unknown;
    method: string;
    url: string;
  }) {
    super(input.message);
    this.name = "CyanrexApiError";
    this.status = input.status;
    this.details = input.details;
    this.method = input.method;
    this.url = input.url;
  }
}

export class CyanrexClient {
  readonly baseUrl: string;
  private readonly fetcher: FetchLike;
  private readonly credentials: RequestCredentials;
  private readonly defaultHeaders: Record<string, string>;
  private readonly csrfOrigin: string | null;
  private sessionCookie: string | null;

  constructor(baseUrl: string, options: CyanrexClientOptions = {}) {
    const normalizedBaseUrl = baseUrl.trim().replace(/\/+$/, "");
    if (!normalizedBaseUrl) {
      throw new TypeError("CyanrexClient requires a non-empty base URL");
    }
    if (!options.fetch && typeof globalThis.fetch !== "function") {
      throw new TypeError("CyanrexClient requires a Fetch API implementation");
    }

    this.baseUrl = normalizedBaseUrl;
    this.fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.credentials = options.credentials ?? "include";
    this.defaultHeaders = { ...options.headers };
    this.csrfOrigin = options.csrfOrigin?.replace(/\/+$/, "") ?? null;
    this.sessionCookie = normalizeSessionCookie(options.sessionCookie ?? null);
  }

  readonly system = {
    info: (options?: RequestOptions) =>
      this.get<SystemInfo>("/", undefined, options),
    health: (options?: RequestOptions) =>
      this.get<HealthResponse>("/health", undefined, options),
    openapi: (options?: RequestOptions) =>
      this.get<OpenApiDocument>("/openapi.json", undefined, options),
  };

  readonly auth = {
    me: (options?: RequestOptions) => this.get<SessionResponse>("/auth/me", undefined, options),
    login: (request: LoginRequest, options?: RequestOptions) =>
      this.post<LoginResponse>("/auth/login", request, options),
    logout: (options?: RequestOptions) =>
      this.post<ApiMessage | undefined>("/auth/logout", undefined, options),
    register: (request: RegisterRequest, options?: RequestOptions) =>
      this.post<TotpBootstrapResponse>("/auth/register", request, options),
    bootstrapTotp: (request: TotpBootstrapRequest, options?: RequestOptions) =>
      this.post<TotpBootstrapResponse>("/auth/totp/bootstrap", request, options),
    changePassword: (request: ChangePasswordRequest, options?: RequestOptions) =>
      this.post<ApiMessage>("/auth/password/change", request, options),
    deleteAccount: (request: DeleteAccountRequest, options?: RequestOptions) =>
      this.post<ApiMessage>("/auth/delete", request, options),
  };

  readonly modules = {
    list: (options?: RequestOptions) => this.get<ModuleInfo[]>("/modules", undefined, options),
    start: (name: string, options?: RequestOptions) =>
      this.post<ModuleInfo>("/modules/start", { name }, options),
    stop: (name: string, options?: RequestOptions) =>
      this.post<ModuleInfo>("/modules/stop", { name }, options),
    headers: {
      catalog: (options?: RequestOptions) =>
        this.get<{ headers: HeaderModuleItem[] }>(
          "/modules/c-headers/catalog",
          undefined,
          options,
        ),
      selected: (options?: RequestOptions) =>
        this.get<{ selected_headers: SelectedHeaderMetadata[] }>(
          "/modules/c-headers/selected-metadata",
          undefined,
          options,
        ),
      download: (id: string, options?: RequestOptions) =>
        this.post<ApiMessage>("/modules/c-headers/download", { id }, options),
      select: (id: string, selected: boolean, options?: RequestOptions) =>
        this.post<ApiMessage>("/modules/c-headers/select", { id, selected }, options),
      delete: (id: string, options?: RequestOptions) =>
        this.post<ApiMessage>("/modules/c-headers/delete", { id }, options),
    },
  };

  readonly command = {
    dispatch: (request: CommandRequest, options?: RequestOptions) =>
      this.post<CommandResponse>("/command", request, options),
  };

  readonly events = {
    list: (query?: EventQuery, options?: RequestOptions) =>
      this.get<EventRecord[]>("/events", query, options),
    unreadCount: (options?: RequestOptions) =>
      this.get<{ unread: number }>("/events/unread-count", undefined, options),
    markRead: (options?: RequestOptions) =>
      this.post<{ ok: boolean }>("/events/mark-read", undefined, options),
    delete: (query?: EventExportQuery, options?: RequestOptions) =>
      this.post<{ ok: boolean; deleted: number }>("/events/delete", undefined, options, query),
    export: (query?: EventExportQuery, options?: RequestOptions) =>
      this.download("/events/export", query, options),
    websocketUrl: () => this.websocketUrl("/ws/events"),
  };

  readonly settings = {
    events: {
      get: (options?: RequestOptions) =>
        this.get<EventSettings>("/settings/events", undefined, options),
      update: (settings: EventSettings, options?: RequestOptions) =>
        this.post<UpdateEventSettingsResponse>("/settings/events", settings, options),
    },
    compiler: {
      get: (options?: RequestOptions) =>
        this.get<CompilerSettings>("/settings/compiler", undefined, options),
      update: (resident: boolean, options?: RequestOptions) =>
        this.post<UpdateCompilerSettingsResponse>("/settings/compiler", { resident }, options),
      performance: (options?: RequestOptions) =>
        this.get<PerformanceMetrics>("/settings/performance", undefined, options),
    },
  };

  readonly scripts = {
    list: (options?: RequestOptions) => this.get<UserScript[]>("/scripts", undefined, options),
    save: (title: string, script: string, options?: RequestOptions) =>
      this.post<SaveScriptResponse>("/scripts/save", { title, script }, options),
    delete: (id: string, options?: RequestOptions) =>
      this.post<ApiMessage>("/scripts/delete", { id }, options),
  };

  readonly learning = {
    labs: (options?: RequestOptions) =>
      this.get<LabProgress[]>("/learning/labs", undefined, options),
    attempts: (options?: RequestOptions) =>
      this.get<LabAttempt[]>("/learning/attempts", undefined, options),
    teacherOverview: (options?: RequestOptions) =>
      this.get<TeacherLearningOverview>("/learning/teacher/overview", undefined, options),
    teacherAttempts: (username: string, limit = 20, options?: RequestOptions) =>
      this.get<TeacherStudentAttempts>(
        "/learning/teacher/attempts",
        { username, limit },
        options,
      ),
  };

  readonly ebpf = {
    check: (code: string, options?: RequestOptions) =>
      this.post<EbpfCheckResponse>("/ebpf/check", { code }, options),
    complete: (code: string, line: number, column: number, options?: RequestOptions) =>
      this.post<EbpfCompletionResponse>("/ebpf/complete", { code, line, column }, options),
    run: (request: EbpfRunRequest, options?: RequestOptions) =>
      this.post<EbpfRunResponse>("/ebpf/run", request, options),
    templates: (options?: RequestOptions) =>
      this.get<EbpfTemplate[]>("/ebpf/templates", undefined, options),
    attachments: (options?: RequestOptions) =>
      this.get<{ pin_paths: string[] }>("/ebpf/attachments", undefined, options),
    attachmentDetails: (options?: RequestOptions) =>
      this.get<{ attachments: EbpfAttachment[] }>(
        "/ebpf/attachments/details",
        undefined,
        options,
      ),
    detach: (pinPath?: string, options?: RequestOptions) =>
      this.post<EbpfDetachResponse>("/ebpf/detach", { pin_path: pinPath }, options),
    checkBackends: (options?: RequestOptions) =>
      this.get<EbpfCheckBackendInventory>("/ebpf/check/backends", undefined, options),
    remoteCheck: {
      submit: (
        request: EbpfRemoteCheckSubmitRequest,
        options?: RequestOptions,
      ) => this.post<EbpfRemoteCheckResponse>("/ebpf/check/remote", request, options),
      status: (jobId: string, options?: RequestOptions) =>
        this.get<EbpfRemoteCheckResponse>(
          "/ebpf/check/remote",
          { job_id: jobId },
          options,
        ),
      cancel: (jobId: string, options?: RequestOptions) =>
        this.post<EbpfRemoteCheckResponse>(
          "/ebpf/check/remote/cancel",
          { job_id: jobId },
          options,
        ),
    },
  };

  readonly runner = {
    status: (options?: RequestOptions) =>
      this.get<RunnerStatus>("/runner/status", undefined, options),
    overview: (options?: RequestOptions) =>
      this.get<RunnerOverview>("/runner/overview", undefined, options),
    agents: (options?: RequestOptions) =>
      this.get<RunnerAgentInventory>("/runner/agents", undefined, options),
    jobs: (options?: RequestOptions) =>
      this.get<RunnerJobInventory>("/runner/jobs", undefined, options),
    submitProbe: (
      request: RunnerProbeRequest,
      options?: RequestOptions,
    ) => this.post<RunnerJob>("/runner/jobs/probe", request, options),
    submitCompileCheck: (
      request: RunnerCompileCheckRequest,
      options?: RequestOptions,
    ) => this.post<RunnerJob>("/runner/jobs/compile-check", request, options),
    cancel: (jobId: string, options?: RequestOptions) =>
      this.post<RunnerJob>("/runner/jobs/cancel", { job_id: jobId }, options),
  };

  readonly helper = {
    environment: (options?: RequestOptions) =>
      this.get<EnvironmentReport>("/helper/environment", undefined, options),
  };

  getSessionCookie(): string | null {
    return this.sessionCookie;
  }

  setSessionCookie(cookie: string | null): void {
    this.sessionCookie = normalizeSessionCookie(cookie);
  }

  async request<T>(
    method: string,
    path: string,
    input: { body?: unknown; query?: Query; signal?: AbortSignal } = {},
  ): Promise<T> {
    const normalizedMethod = method.toUpperCase();
    const url = this.url(path, input.query);
    const headers: Record<string, string> = {
      Accept: "application/json",
      ...this.defaultHeaders,
    };
    if (input.body !== undefined) {
      headers["Content-Type"] = "application/json";
    }
    if (this.csrfOrigin && !isSafeMethod(normalizedMethod)) {
      headers.Origin = this.csrfOrigin;
    }
    if (this.sessionCookie) {
      headers.Cookie = this.sessionCookie;
    }

    const response = await this.fetcher(url, {
      method: normalizedMethod,
      credentials: this.credentials,
      headers,
      body: input.body === undefined ? undefined : JSON.stringify(input.body),
      signal: input.signal,
    });
    this.captureSessionCookie(response);
    const details = await parseBody(response);
    if (!response.ok) {
      throw new CyanrexApiError({
        message: errorMessage(details, response),
        status: response.status,
        details,
        method: normalizedMethod,
        url,
      });
    }
    return details as T;
  }

  private get<T>(path: string, query?: Query, options?: RequestOptions): Promise<T> {
    return this.request<T>("GET", path, { query, signal: options?.signal });
  }

  private post<T>(
    path: string,
    body?: unknown,
    options?: RequestOptions,
    query?: Query,
  ): Promise<T> {
    return this.request<T>("POST", path, { body, query, signal: options?.signal });
  }

  private async download(
    path: string,
    query?: Query,
    options?: RequestOptions,
  ): Promise<ApiDownload> {
    const url = this.url(path, query);
    const headers: Record<string, string> = { ...this.defaultHeaders };
    if (this.sessionCookie) headers.Cookie = this.sessionCookie;
    const response = await this.fetcher(url, {
      method: "GET",
      credentials: this.credentials,
      headers,
      signal: options?.signal,
    });
    this.captureSessionCookie(response);
    if (!response.ok) {
      const details = await parseBody(response);
      throw new CyanrexApiError({
        message: errorMessage(details, response),
        status: response.status,
        details,
        method: "GET",
        url,
      });
    }
    return {
      blob: await response.blob(),
      filename: parseDownloadFilename(response.headers.get("content-disposition")),
      contentType: response.headers.get("content-type"),
    };
  }

  private url(path: string, query?: Query): string {
    const normalizedPath = path.startsWith("/") ? path : `/${path}`;
    const search = new URLSearchParams();
    for (const [key, value] of Object.entries(query ?? {})) {
      if (value !== undefined && value !== null && value !== "") {
        search.set(key, String(value));
      }
    }
    const suffix = search.size > 0 ? `?${search.toString()}` : "";
    return `${this.baseUrl}${normalizedPath}${suffix}`;
  }

  private websocketUrl(path: string): string {
    const url = new URL(this.url(path));
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    return url.toString();
  }

  private captureSessionCookie(response: Response): void {
    const headers = response.headers as Headers & { getSetCookie?: () => string[] };
    const values = headers.getSetCookie?.() ?? [headers.get("set-cookie")].filter(isString);
    const session = values
      .map((value) => value.split(";", 1)[0]?.trim())
      .find((value) => value?.startsWith("cyanrex_session="));
    if (!session) return;
    this.sessionCookie = session === "cyanrex_session=" ? null : session;
  }
}

function isSafeMethod(method: string): boolean {
  return method === "GET" || method === "HEAD" || method === "OPTIONS";
}

function normalizeSessionCookie(cookie: string | null): string | null {
  const normalized = cookie?.split(";", 1)[0]?.trim() ?? "";
  if (!normalized) return null;
  return normalized.startsWith("cyanrex_session=")
    ? normalized
    : `cyanrex_session=${normalized}`;
}

async function parseBody(response: Response): Promise<unknown> {
  if (response.status === 204 || response.status === 205) return undefined;
  const text = await response.text();
  if (!text) return undefined;
  const contentType = response.headers.get("content-type") ?? "";
  if (contentType.includes("json")) {
    try {
      return JSON.parse(text) as unknown;
    } catch {
      return text;
    }
  }
  return text;
}

function errorMessage(details: unknown, response: Response): string {
  if (details && typeof details === "object" && "message" in details) {
    const message = (details as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) return message;
  }
  if (typeof details === "string" && details.trim()) return details;
  return `HTTP ${response.status}${response.statusText ? ` ${response.statusText}` : ""}`;
}

function parseDownloadFilename(disposition: string | null): string | null {
  if (!disposition) return null;
  const encoded = disposition.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  if (encoded) {
    try {
      return decodeURIComponent(encoded);
    } catch {
      return encoded;
    }
  }
  return disposition.match(/filename="?([^";]+)"?/i)?.[1] ?? null;
}

function isString(value: string | null): value is string {
  return typeof value === "string";
}
