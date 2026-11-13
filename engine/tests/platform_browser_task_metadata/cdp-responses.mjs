// Passive, session-owned response bodies. Never replay a request to recover missing evidence.
import { loadingFailurePart, readObservedJson } from "./observations.mjs";

function failure(part) {
  const error = new Error("private browser response observation failed");
  error.observationPart = part;
  return error;
}

export async function createCdpResponses(session, { timeoutMs = 10000 } = {}) {
  const pending = new Set(), bound = new Map(), queues = new Set();
  let closed = false, closing, poisoned = null;
  const waitForNext = queue => {
    if (queue.items.every(item => !pending.has(item))) {
      clearTimeout(queue.timer); queue.timer = null; return;
    }
    const item = queue.items[queue.next];
    if (!item || item.timer !== null || queue.items.slice(0, queue.next).some(item => pending.has(item))) return;
    // Only the next unsent request waits, after every prior confirmation. A legal
    // preceding response must not consume this request's send or response budget.
    item.timer = setTimeout(() => fail(item, "event-timeout"), timeoutMs);
  };
  const settle = (item, error, value) => {
    if (!pending.delete(item)) return;
    clearTimeout(item.timer);
    if (item.id !== null) bound.delete(item.id);
    if (error) item.reject(error); else item.resolve(value);
    if (item.queue && !error && !poisoned && !closed) waitForNext(item.queue);
  };
  const poison = part => {
    poisoned ??= part;
    for (const queue of queues) { clearTimeout(queue.timer); queue.timer = null; }
    for (const item of pending) settle(item, failure(poisoned));
  };
  const fail = (item, part) => {
    if (!pending.has(item)) return;
    if (item.queue) poison(part); else settle(item, failure(part));
  };
  const matches = (item, url, method) => item.url === url && item.method === method;
  const arm = (url, method) => {
    if (closed || poisoned || [...pending].some(item => matches(item, url, method)))
      throw failure("event-failed");
    // A completed batch remains an overrun guard until the next explicit arm of this key.
    // Old single-response expect has no retained guard and keeps its original semantics.
    for (const queue of queues) if (matches(queue, url, method)) queues.delete(queue);
  };
  const create = (url, method, queue = null) => {
    let resolve, reject;
    const incoming = new Promise((yes, no) => { resolve = yes; reject = no; });
    // Register rejection handling before the caller adds its observation-ledger check.
    incoming.catch(() => {});
    const item = { url, method, queue, resolve, reject, id: null, status: null, reading: false, timer: null };
    // Preserve expect()'s arm-time deadline; queued responses start theirs on binding.
    if (!queue) item.timer = setTimeout(() => fail(item, "event-timeout"), timeoutMs);
    pending.add(item);
    return { item, incoming };
  };
  const requested = event => {
    if (poisoned) return;
    // A redirect reuses its request id; it must not become another matching response.
    if (bound.has(event.requestId)) { fail(bound.get(event.requestId), "event-request-reused"); return; }
    for (const queue of queues) {
      if (queue.ids.has(event.requestId)) { poison("event-request-reused"); return; }
    }
    for (const queue of queues) {
      if (!matches(queue, event.request.url, event.request.method)) continue;
      if (event.redirectResponse) { poison("event-redirect"); return; }
      const item = queue.items[queue.next];
      if (!item) { poison("event-request-duplicate"); return; }
      queue.next++;
      clearTimeout(item.timer);
      item.timer = setTimeout(() => fail(item, "event-timeout"), timeoutMs);
      item.id = event.requestId; queue.ids.add(item.id); bound.set(item.id, item); return;
    }
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
      value => settle(item, null, value), error => fail(item, error.observationPart));
  };
  const failed = event => {
    const item = bound.get(event.requestId);
    if (item) fail(item, loadingFailurePart(event,
      item.reading ? "body" : item.status !== null ? "response" : "request"));
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
      arm(url, method);
      return create(url, method).incoming;
    },
    expectMany(url, method, count) {
      if (!Number.isInteger(count) || count < 1 || count > 32) throw failure("event-failed");
      arm(url, method);
      const queue = { url, method, items: [], ids: new Set(), next: 0, timer: null };
      // Each slot may spend one full send wait and one full response wait. Keep a
      // conservative absolute ceiling too; success removes timers, not the overrun guard.
      queue.timer = setTimeout(() => poison("event-timeout"), (2 * count + 1) * timeoutMs);
      queues.add(queue);
      const incoming = Array.from({ length: count }, () => {
        const { item, incoming } = create(url, method, queue);
        queue.items.push(item); return incoming;
      });
      waitForNext(queue);
      return Object.freeze(incoming);
    },
    check() {
      if (poisoned || closed) throw failure(poisoned ?? "event-session-closed");
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
