import type { EbpfAttachmentDetail, EbpfDetachResponse, EbpfRunResponse } from "./models";

export const RUNTIME_MUTATION_TIMEOUT_MS = 330_000;
export const ATTACHMENT_READ_TIMEOUT_MS = 20_000;

export class RuntimeHttpError extends Error {
  readonly status: number;
  readonly payload: unknown;
  constructor(status: number, payload: unknown, message: string) {
    super(message); this.name = "RuntimeHttpError"; this.status = status; this.payload = payload;
  }
}

// Browser waiting limits are not a server rollback or kernel-cleanup guarantee.
export async function requestRuntimeJson(
  url: string, parent: AbortSignal, body?: unknown, timeoutMs = RUNTIME_MUTATION_TIMEOUT_MS,
): Promise<unknown> {
  parent.throwIfAborted();
  const controller = new AbortController();
  const cancel = () => controller.abort(parent.reason);
  parent.addEventListener("abort", cancel, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException("Runtime request timed out", "TimeoutError")), timeoutMs);
  try {
    const response = await fetch(url, {
      method: body === undefined ? "GET" : "POST", credentials: "include", cache: "no-store", redirect: "error",
      signal: controller.signal,
      ...(body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }),
    });
    const payload = await response.json();
    controller.signal.throwIfAborted();
    if (!response.ok) throw new RuntimeHttpError(response.status, payload,
      record(payload) && typeof payload.message === "string" ? payload.message : `HTTP ${response.status}`);
    return payload;
  } finally { clearTimeout(timer); parent.removeEventListener("abort", cancel); }
}

const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const strings = (value: unknown): value is string[] => Array.isArray(value) && value.every(item => typeof item === "string");
const lines = (value: unknown) => Array.isArray(value) && value.every(item => Number.isInteger(item) && item > 0);
const optionalString = (value: unknown) => value == null || typeof value === "string";

export function parseRunResult(value: unknown): EbpfRunResponse {
  if (!record(value) || typeof value.success !== "boolean"
    || !["stage", "message", "compile_stdout", "compile_stderr", "load_stdout", "load_stderr"].every(key => typeof value[key] === "string")
    || !optionalString(value.pin_path)) throw new Error("Invalid runtime response");
  const debug = value.debug;
  if (debug != null && (!record(debug) || typeof debug.mode !== "string" || !optionalString(debug.session_id)
    || !lines(debug.requested_lines) || !lines(debug.instrumented_lines) || !Array.isArray(debug.rejected)
    || !debug.rejected.every(item => record(item) && Number.isInteger(item.line) && Number(item.line) > 0 && typeof item.reason === "string"))) {
    throw new Error("Invalid runtime debug response");
  }
  return value as EbpfRunResponse;
}

export function parseDetachResult(value: unknown): EbpfDetachResponse {
  if (!record(value) || typeof value.ok !== "boolean" || typeof value.message !== "string"
    || !strings(value.detached) || value.detached.some(item => !item.trim())
    || new Set(value.detached).size !== value.detached.length
    || (value.clean !== undefined && typeof value.clean !== "boolean")
    || (value.safety_notes !== undefined && !strings(value.safety_notes))) throw new Error("Invalid detach response");
  return value as EbpfDetachResponse;
}

export function parseAttachmentDetails(value: unknown): EbpfAttachmentDetail[] {
  if (!record(value) || !Array.isArray(value.attachments) || !value.attachments.every(item => record(item)
    && typeof item.pin_path === "string" && item.pin_path.trim()
    && typeof item.source === "string" && typeof item.program_name === "string")
    || new Set(value.attachments.map(item => item.pin_path)).size !== value.attachments.length) {
    throw new Error("Invalid attachment inventory");
  }
  return value.attachments as EbpfAttachmentDetail[];
}
