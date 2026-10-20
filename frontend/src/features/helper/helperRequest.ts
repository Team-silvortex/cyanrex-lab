import { PRIVATE_READ_TIMEOUT_MS, requestPrivate } from "../../transport/privateRequest";

export const HELPER_CHECK_TIMEOUT_MS = PRIVATE_READ_TIMEOUT_MS;
export const MAX_HELPER_RESPONSE_BYTES = 128 * 1024;
export type EnvironmentReport = {
  overall_ok: boolean; generated_at: string; runtime_mode: "native-linux" | "wsl2" | "docker";
  runtime_guidance: string; checks: { name: string; ok: boolean; detail: string }[];
};
export type RunnerStatus = {
  mode: string; isolation: string; instance_id: string; max_concurrent: number; max_per_user: number;
  active_total: number; active_for_current_user: number; available_slots: number; execution_timeout_seconds: number;
};
export type HelperSnapshot = { report: EnvironmentReport; runner: RunnerStatus };

const invalid = () => new Error("Invalid helper response");
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number, nonempty = false): string {
  if (typeof value !== "string" || value.length > max || (nonempty && !value.trim())) throw invalid();
  return value;
}
function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") throw invalid();
  return value;
}
function integer(value: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) throw invalid();
  return value;
}
function date(value: unknown): string {
  const result = text(value, 64);
  const parts = result.match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(Z|[+-]\d{2}:\d{2})$/);
  if (!parts || !Number.isFinite(Date.parse(result))) throw invalid();
  const [, year, month, day, hour, minute, second, zone] = parts;
  const y = Number(year), m = Number(month), d = Number(day);
  const leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (m < 1 || m > 12 || d < 1 || d > days[m - 1] || Number(hour) > 23 || Number(minute) > 59 || Number(second) > 59
    || (zone !== "Z" && (Number(zone.slice(1, 3)) > 23 || Number(zone.slice(4)) > 59))) throw invalid();
  return result;
}

export function parseEnvironmentReport(value: unknown): EnvironmentReport {
  const v = record(value);
  if (!Array.isArray(v.checks) || v.checks.length < 1 || v.checks.length > 32
    || !["native-linux", "wsl2", "docker"].includes(v.runtime_mode as string)) throw invalid();
  const checks = v.checks.map(value => {
    const check = record(value);
    return { name: text(check.name, 128, true), ok: boolean(check.ok), detail: text(check.detail, 8192) };
  });
  const overall_ok = boolean(v.overall_ok);
  if (overall_ok !== checks.every(check => check.ok) || new Set(checks.map(check => check.name)).size !== checks.length) throw invalid();
  return { overall_ok, generated_at: date(v.generated_at), runtime_mode: v.runtime_mode as EnvironmentReport["runtime_mode"],
    runtime_guidance: text(v.runtime_guidance, 2048), checks };
}

export function parseRunnerStatus(value: unknown): RunnerStatus {
  const v = record(value), max_concurrent = integer(v.max_concurrent, 1), max_per_user = integer(v.max_per_user, 1, max_concurrent);
  const active_total = integer(v.active_total, 0, max_concurrent), active_for_current_user = integer(v.active_for_current_user, 0, Math.min(max_per_user, active_total));
  const available_slots = integer(v.available_slots, 0, max_concurrent);
  if (available_slots !== max_concurrent - active_total) throw invalid();
  return { mode: text(v.mode, 128, true), isolation: text(v.isolation, 128, true), instance_id: text(v.instance_id, 256, true),
    max_concurrent, max_per_user, active_total, active_for_current_user, available_slots,
    execution_timeout_seconds: integer(v.execution_timeout_seconds, 1) };
}

function read<T>(url: string, parent: AbortSignal, decode: (value: unknown) => T): Promise<T> {
  return requestPrivate(url, parent, async (response, signal) => {
    const length = response.headers.get("content-length");
    if (!response.ok || response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "application/json"
      || (length !== null && (!/^\d+$/.test(length) || Number(length) > MAX_HELPER_RESPONSE_BYTES)) || !response.body) {
      void response.body?.cancel().catch(() => undefined);
      throw new Error("Helper request failed");
    }
    const reader = response.body.getReader(), utf8 = new TextDecoder("utf-8", { fatal: true });
    let cancelled = false;
    const cancel = () => { if (!cancelled) { cancelled = true; void reader.cancel().catch(() => undefined); } };
    signal.addEventListener("abort", cancel, { once: true });
    let bytes = 0, content = "";
    try {
      signal.throwIfAborted();
      while (true) {
        const { value, done } = await reader.read();
        signal.throwIfAborted();
        if (done) break;
        bytes += value.byteLength;
        if (bytes > MAX_HELPER_RESPONSE_BYTES) throw invalid();
        content += utf8.decode(value, { stream: true });
      }
      content += utf8.decode();
      signal.throwIfAborted();
      return decode(JSON.parse(content));
    } catch (error) { cancel(); throw error; }
    finally { signal.removeEventListener("abort", cancel); reader.releaseLock(); }
  }, { timeoutMs: HELPER_CHECK_TIMEOUT_MS, timeoutMessage: "Helper request timed out" });
}

/** One explicit browser observation, not a deployment action or cancellation of Engine work. */
export async function readHelperSnapshot(engineUrl: string, parent: AbortSignal): Promise<HelperSnapshot> {
  parent.throwIfAborted();
  const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
  parent.addEventListener("abort", cancel, { once: true });
  try {
    const [report, runner] = await Promise.all([
      read(`${engineUrl}/helper/environment`, controller.signal, parseEnvironmentReport),
      read(`${engineUrl}/runner/status`, controller.signal, parseRunnerStatus),
    ]);
    parent.throwIfAborted();
    return { report, runner };
  } finally {
    // Failure of either half also ends the sibling's browser wait. Never publish half a pair.
    controller.abort(); parent.removeEventListener("abort", cancel);
  }
}
