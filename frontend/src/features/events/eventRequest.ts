export const EVENT_REQUEST_TIMEOUT_MS = 20_000;
export const EVENT_UNREAD_TIMEOUT_MS = 10_000;
export const UNREAD_CHANGED = "cyanrex-events-unread-changed";

export class EventHttpError extends Error {
  readonly status: number;
  constructor(status: number) { super(`HTTP ${status}`); this.name = "EventHttpError"; this.status = status; }
}

// A browser deadline/cancellation ends waiting, not a server mutation or persistence transaction.
export async function requestEvent<T>(url: string, parent: AbortSignal, decode: (response: Response) => Promise<T>,
  method = "GET", timeout = EVENT_REQUEST_TIMEOUT_MS): Promise<T> {
  parent.throwIfAborted();
  const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
  parent.addEventListener("abort", cancel, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException("Event request timed out", "TimeoutError")), timeout);
  try {
    const response = await fetch(url, { method, signal: controller.signal, credentials: "include", cache: "no-store", redirect: "error" });
    controller.signal.throwIfAborted();
    if (!response.ok) { void response.body?.cancel().catch(() => undefined); throw new EventHttpError(response.status); }
    const result = await decode(response);
    controller.signal.throwIfAborted();
    return result;
  } finally { clearTimeout(timer); parent.removeEventListener("abort", cancel); }
}

const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const count = (value: unknown): value is number => Number.isSafeInteger(value) && Number(value) >= 0;
export function parseUnread(value: unknown): number {
  if (!record(value) || !count(value.unread)) throw new Error("Invalid unread count");
  return value.unread;
}
export function parseReadAcknowledgement(value: unknown): void {
  if (!record(value) || value.ok !== true) throw new Error("Unconfirmed read acknowledgement");
}
export function parseEventDeletion(value: unknown): number {
  if (!record(value) || value.ok !== true || !count(value.deleted)) throw new Error("Unconfirmed event deletion");
  return value.deleted;
}
export async function decodeEventExport(response: Response, format: "json" | "csv") {
  const type = response.headers.get("content-type")?.split(";")[0].trim().toLowerCase();
  if (type !== (format === "json" ? "application/json" : "text/csv")) {
    void response.body?.cancel().catch(() => undefined); throw new Error("Unexpected event export format");
  }
  const candidate = response.headers.get("content-disposition")?.match(/filename="([^"]+)"/i)?.[1];
  const filename = candidate && /^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$/.test(candidate) && candidate.endsWith(`.${format}`)
    ? candidate : `cyanrex-events.${format}`;
  return { blob: await response.blob(), filename };
}
export function notifyUnreadChanged(engineUrl: string) {
  window.dispatchEvent(new CustomEvent(UNREAD_CHANGED, { detail: { engineUrl } }));
}
