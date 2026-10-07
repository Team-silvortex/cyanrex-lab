export const PRIVATE_READ_TIMEOUT_MS = 10_000;
export const PRIVATE_WRITE_TIMEOUT_MS = 20_000;

type PrivateRequestOptions = {
  method?: string;
  body?: unknown;
  timeoutMs: number;
  timeoutMessage: string;
};

// Own one browser wait, not server authority, a transaction, retries or rollback.
export async function requestPrivate<T>(
  url: string, parent: AbortSignal, decode: (response: Response, signal: AbortSignal) => Promise<T>,
  options: PrivateRequestOptions,
): Promise<T> {
  parent.throwIfAborted();
  const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
  let rejectAbort: (reason: unknown) => void = () => undefined;
  const interrupted = new Promise<never>((_resolve, reject) => { rejectAbort = reject; });
  const aborted = () => rejectAbort(controller.signal.reason);
  parent.addEventListener("abort", cancel, { once: true });
  controller.signal.addEventListener("abort", aborted, { once: true });
  const timer = setTimeout(() => controller.abort(new DOMException(options.timeoutMessage, "TimeoutError")), options.timeoutMs);
  try {
    const work = (async () => {
      const response = await fetch(url, {
        method: options.method ?? (options.body === undefined ? "GET" : "POST"),
        credentials: "include", cache: "no-store", redirect: "error", signal: controller.signal,
        ...(options.body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(options.body) }),
      });
      if (controller.signal.aborted) {
        void response.body?.cancel().catch(() => undefined);
        controller.signal.throwIfAborted();
      }
      const result = await decode(response, controller.signal);
      controller.signal.throwIfAborted();
      return result;
    })();
    return await Promise.race([work, interrupted]);
  } finally {
    clearTimeout(timer);
    parent.removeEventListener("abort", cancel);
    controller.signal.removeEventListener("abort", aborted);
  }
}

// Strict JSON is a selected response policy; Event exports and Runtime errors use their own decoders.
export function requestPrivateJson<T>(
  url: string, parent: AbortSignal, decode: (value: unknown) => T, body?: unknown,
  timeout = body === undefined ? PRIVATE_READ_TIMEOUT_MS : PRIVATE_WRITE_TIMEOUT_MS, label = "Engine",
): Promise<T> {
  return requestPrivate(url, parent, async (response, signal) => {
    if (!response.ok || response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "application/json") {
      void response.body?.cancel().catch(() => undefined);
      throw new Error(`${label} request failed`);
    }
    const value: unknown = await response.json();
    signal.throwIfAborted();
    return decode(value);
  }, { body, timeoutMs: timeout, timeoutMessage: `${label} request timed out` });
}
