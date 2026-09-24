export type EventSettings = { max_records: number; overflow_policy: "drop_oldest" | "drop_new" };
export type CompilerSettings = { resident: boolean; strategy: "resident_cache" | "on_demand" };
export const SETTINGS_READ_TIMEOUT_MS = 10_000;
export const SETTINGS_SAVE_TIMEOUT_MS = 20_000;

const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const invalid = () => new Error("Invalid settings response");

export function parseEventSettings(value: unknown): EventSettings {
  if (!record(value) || !Number.isSafeInteger(value.max_records) || Number(value.max_records) < 50
    || Number(value.max_records) > 50000 || (value.overflow_policy !== "drop_oldest" && value.overflow_policy !== "drop_new")) throw invalid();
  return { max_records: value.max_records as number, overflow_policy: value.overflow_policy as EventSettings["overflow_policy"] };
}

export function normalizeEventDraft(value: EventSettings): EventSettings {
  if (!Number.isSafeInteger(value.max_records)) throw invalid();
  return parseEventSettings({ ...value, max_records: Math.max(50, Math.min(50000, value.max_records)) });
}

export function parseCompilerSettings(value: unknown): CompilerSettings {
  if (!record(value) || typeof value.resident !== "boolean"
    || value.strategy !== (value.resident ? "resident_cache" : "on_demand")) throw invalid();
  return { resident: value.resident, strategy: value.strategy as CompilerSettings["strategy"] };
}

export function parseEventSettingsSaved(value: unknown, expected: EventSettings): EventSettings {
  if (!record(value) || value.ok !== true) throw invalid();
  const settings = parseEventSettings(value.settings);
  if (settings.max_records !== expected.max_records || settings.overflow_policy !== expected.overflow_policy) throw invalid();
  return settings;
}

export function parseCompilerSettingsSaved(value: unknown, resident: boolean): CompilerSettings {
  if (!record(value) || value.ok !== true) throw invalid();
  const settings = parseCompilerSettings(value.settings);
  if (settings.resident !== resident) throw invalid();
  return settings;
}

// A deadline ends browser waiting, not an Engine mutation. Never retry a settings write here.
export async function requestSettings<T>(url: string, parent: AbortSignal, decode: (value: unknown) => T,
  body?: unknown, timeout = body === undefined ? SETTINGS_READ_TIMEOUT_MS : SETTINGS_SAVE_TIMEOUT_MS): Promise<T> {
  parent.throwIfAborted();
  const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
  let rejectAbort: (reason: unknown) => void = () => undefined;
  const interrupted = new Promise<never>((_resolve, reject) => { rejectAbort = reject; });
  const aborted = () => rejectAbort(controller.signal.reason);
  parent.addEventListener("abort", cancel, { once: true });
  controller.signal.addEventListener("abort", aborted, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException("Settings request timed out", "TimeoutError")), timeout);
  try {
    const work = (async () => {
      const response = await fetch(url, { method: body === undefined ? "GET" : "POST",
        credentials: "include", cache: "no-store", redirect: "error", signal: controller.signal,
        ...(body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }) });
      if (controller.signal.aborted || !response.ok || response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "application/json") {
        void response.body?.cancel().catch(() => undefined);
        controller.signal.throwIfAborted();
        throw new Error("Settings request failed");
      }
      const value: unknown = await response.json();
      controller.signal.throwIfAborted();
      return decode(value);
    })();
    return await Promise.race([work, interrupted]);
  } finally {
    clearTimeout(timer);
    parent.removeEventListener("abort", cancel);
    controller.signal.removeEventListener("abort", aborted);
  }
}
