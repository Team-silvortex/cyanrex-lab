// Passive, session-owned response bodies. Never replay a request to recover missing evidence.
import { readObservedJson } from "./observations.mjs";

function failure(part) {
  const error = new Error("private browser response observation failed");
  error.observationPart = part;
  return error;
}

export async function createCdpResponses(session, { timeoutMs = 10000 } = {}) {
  const pending = new Set(), bound = new Map();
  let closed = false, closing;
  const settle = (item, error, value) => {
    if (!pending.delete(item)) return;
    clearTimeout(item.timer);
    if (item.id !== null) bound.delete(item.id);
    if (error) item.reject(error); else item.resolve(value);
  };
  const fail = (item, part) => settle(item, failure(part));
  const requested = event => {
    // A redirect reuses its request id; it must not become another matching response.
    if (bound.has(event.requestId)) { fail(bound.get(event.requestId), "event-request-reused"); return; }
    for (const item of pending) {
      if (event.request.url !== item.url || event.request.method !== item.method) continue;
      if (item.id !== null) { fail(item, "event-request-duplicate"); return; }
      if (event.redirectResponse) { fail(item, "event-redirect"); return; }
      item.id = event.requestId; bound.set(item.id, item); return;
    }
  };
  const received = event => {
    const item = bound.get(event.requestId);
    if (!item) return;
    const { url, status } = event.response;
    if (item.status !== null) { fail(item, "event-response-duplicate"); return; }
    if (url !== item.url) { fail(item, "event-response-mismatch"); return; }
    if (!Number.isInteger(status) || status < 100 || status > 599) {
      fail(item, "event-status-invalid"); return;
    }
    item.status = status;
  };
  const finished = event => {
    const item = bound.get(event.requestId);
    if (!item) return;
    if (item.status === null) { fail(item, "event-finished-before-response"); return; }
    if (item.reading) { fail(item, "event-finished-duplicate"); return; }
    item.reading = true;
    // Start the only body read from loadingFinished, not a later DOM-state callback.
    // The public Playwright response helper can recover an empty body with a new GET;
    // this facade has no such transport fallback, and all failures remain failures.
    const response = { status: () => item.status, text: async () => {
      const result = await session.send("Network.getResponseBody", { requestId: item.id });
      if (typeof result.body !== "string" || typeof result.base64Encoded !== "boolean")
        throw failure("body-failed");
      return result.base64Encoded
        ? new TextDecoder("utf-8", { fatal: true }).decode(Buffer.from(result.body, "base64"))
        : result.body;
    } };
    readObservedJson(Promise.resolve(response)).then(
      value => settle(item, null, value), error => settle(item, error));
  };
  const failed = event => {
    const item = bound.get(event.requestId); if (item) fail(item, "event-loading-failed");
  };
  const disconnected = () => {
    closed = true;
    for (const item of pending) fail(item, "event-session-closed");
  };
  const listeners = { "Network.requestWillBeSent": requested, "Network.responseReceived": received,
    "Network.loadingFinished": finished, "Network.loadingFailed": failed, close: disconnected };
  for (const [event, handler] of Object.entries(listeners)) session.on(event, handler);
  try { await session.send("Network.enable"); }
  catch {
    for (const [event, handler] of Object.entries(listeners)) session.off(event, handler);
    throw failure("event-failed");
  }
  return {
    expect(url, method) {
      if (closed || [...pending].some(item => item.url === url && item.method === method))
        throw failure("event-failed");
      let resolve, reject;
      const incoming = new Promise((yes, no) => { resolve = yes; reject = no; });
      // Handle rejection immediately even if the observation ledger runs next microtask.
      incoming.catch(() => {});
      const item = { url, method, resolve, reject, id: null, status: null, reading: false };
      item.timer = setTimeout(() => settle(item, failure("event-timeout")), timeoutMs);
      pending.add(item);
      return incoming;
    },
    async close() {
      if (closing) return closing;
      const attached = !closed;
      disconnected();
      for (const [event, handler] of Object.entries(listeners)) session.off(event, handler);
      closing = attached ? session.detach().catch(() => { throw failure("event-failed"); })
        : Promise.resolve();
      await closing;
    },
  };
}
