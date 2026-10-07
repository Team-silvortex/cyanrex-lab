import { requestPrivateJson, PRIVATE_READ_TIMEOUT_MS, PRIVATE_WRITE_TIMEOUT_MS } from "../../transport/privateRequest";

export type EventSettings = { max_records: number; overflow_policy: "drop_oldest" | "drop_new" };
export type CompilerSettings = { resident: boolean; strategy: "resident_cache" | "on_demand" };
export const SETTINGS_READ_TIMEOUT_MS = PRIVATE_READ_TIMEOUT_MS;
export const SETTINGS_SAVE_TIMEOUT_MS = PRIVATE_WRITE_TIMEOUT_MS;

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
export function requestSettings<T>(url: string, parent: AbortSignal, decode: (value: unknown) => T,
  body?: unknown, timeout = body === undefined ? SETTINGS_READ_TIMEOUT_MS : SETTINGS_SAVE_TIMEOUT_MS): Promise<T> {
  return requestPrivateJson(url, parent, decode, body, timeout, "Settings");
}
