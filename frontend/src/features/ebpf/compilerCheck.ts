import type { EbpfCheckResponse, EbpfCompilerTarget, EbpfRemoteCheckResponse } from "./models";

// Bounds the whole request, including submission, response parsing and polling.
// Local Engine checks already have a 15-second driver deadline; allow transport time.
const LOCAL_CHECK_TIMEOUT_MS = 20_000;
const REMOTE_CHECK_TIMEOUT_MS = 35_000;
const PRIVATE_REQUEST = { credentials: "include", cache: "no-store", redirect: "error" } as const;

export async function runCompilerCheck(
  code: string,
  engineUrl: string,
  target: EbpfCompilerTarget,
  parentSignal: AbortSignal,
): Promise<EbpfCheckResponse> {
  parentSignal.throwIfAborted();
  const controller = new AbortController();
  const onAbort = () => controller.abort(parentSignal.reason);
  parentSignal.addEventListener("abort", onAbort, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException("Compiler check timed out", "TimeoutError")),
    target === "local" ? LOCAL_CHECK_TIMEOUT_MS : REMOTE_CHECK_TIMEOUT_MS);
  const { signal } = controller;
  try {
    const result = target.startsWith("agent:")
      ? await runRemoteCompilerCheck(code, engineUrl, target.slice(6), signal)
      : await fetchJson<EbpfCheckResponse>(`${engineUrl}/ebpf/check`, {
        method: "POST", headers: { "Content-Type": "application/json" },
        signal, body: JSON.stringify({ code }),
      });
    signal.throwIfAborted();
    return result;
  } finally {
    clearTimeout(timer);
    parentSignal.removeEventListener("abort", onAbort);
  }
}

async function runRemoteCompilerCheck(
  code: string, engineUrl: string, agentId: string, signal: AbortSignal,
): Promise<EbpfCheckResponse> {
  let jobId = "";
  let completed = false;
  try {
    const submitted = await fetchJson<EbpfRemoteCheckResponse>(`${engineUrl}/ebpf/check/remote`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      signal, body: JSON.stringify({ code, agent_id: agentId, program_name: "inline-check" }),
    });
    // Keep the ID before checking cancellation so a late submit can still be cleaned up.
    jobId = submitted.job_id;
    signal.throwIfAborted();
    if (!jobId) throw new Error("remote check response did not contain a job ID");
    while (true) {
      const status = await fetchJson<EbpfRemoteCheckResponse>(
        `${engineUrl}/ebpf/check/remote?job_id=${encodeURIComponent(jobId)}`, { signal },
      );
      signal.throwIfAborted();
      if (["succeeded", "failed", "cancelled", "expired"].includes(status.state)) {
        completed = true;
        if (["cancelled", "expired"].includes(status.state) || !status.result) {
          throw new Error(status.message || `remote check ended as ${status.state}`);
        }
        return status.result;
      }
      await abortableDelay(500, signal);
    }
  } finally {
    if (jobId && !completed) void cancelRemoteCheck(engineUrl, jobId);
  }
}

async function fetchJson<T>(url: string, init: RequestInit): Promise<T> {
  const response = await fetch(url, { ...PRIVATE_REQUEST, ...init });
  if (!response.ok) {
    const payload = await response.json().catch(() => ({})) as { message?: string } | null;
    throw new Error(payload?.message || `HTTP ${response.status}`);
  }
  return (await response.json()) as T;
}

async function cancelRemoteCheck(engineUrl: string, jobId: string) {
  try {
    await fetch(`${engineUrl}/ebpf/check/remote/cancel`, {
      ...PRIVATE_REQUEST, method: "POST", headers: { "Content-Type": "application/json" },
      keepalive: true, signal: AbortSignal.timeout(10_000), body: JSON.stringify({ job_id: jobId }),
    });
  } catch {
    // Best effort, never a rollback guarantee. Unknown/lost IDs are left to the
    // server's user-queue expiry or claimed lease deadline; no automatic retry.
  }
}

function abortableDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
  signal.throwIfAborted();
  return new Promise((resolve, reject) => {
    const onAbort = () => { clearTimeout(timer); reject(signal.reason); };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}
