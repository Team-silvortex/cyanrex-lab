import { getEngineUrl } from "../../config/runtime";

export async function classroomRequest<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const response = await fetch(`${getEngineUrl()}${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: path.startsWith("/classroom/invitations") ? "include" : "omit",
    cache: "no-store", redirect: "error", referrerPolicy: "no-referrer",
    signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(10000)]) : AbortSignal.timeout(10000),
  });
  const payload = await response.json();
  if (!response.ok) throw new Error(payload.message || `HTTP ${response.status}`);
  return payload as T;
}
