import assert from "node:assert/strict";
import test from "node:test";
import { buildEventFilterParams, matchesEventFilters } from "../src/features/events/eventFilters.ts";
import { decodeEventExport, EVENT_REQUEST_TIMEOUT_MS, EVENT_UNREAD_TIMEOUT_MS, EventHttpError,
  parseEventDeletion, parseReadAcknowledgement, parseUnread, requestEvent } from "../src/features/events/eventRequest.ts";

const filters = { categoryFilter: "all", severityFilter: "all", rangePreset: "all", startTime: "", endTime: "" };
const row = { category: "kernel", severity: "error", timestamp: "2026-09-13T00:00:00Z" };
const signal = () => new AbortController().signal;

test("event filters retain category, severity and bounded history but export has no page limit", () => {
  const input = { ...filters, categoryFilter: "kernel", severityFilter: "error", rangePreset: "10m" };
  assert.deepEqual(Object.fromEntries(buildEventFilterParams(input, 200)), { category: "kernel", severity: "error", limit: "200", since_minutes: "10" });
  const query = buildEventFilterParams(input, undefined, "csv"); assert.equal(query.get("format"), "csv"); assert.equal(query.has("limit"), false);
});

test("invalid or reversed time ranges never turn into wider history/export queries", () => {
  for (const edit of [{ categoryFilter: "unknown" }, { severityFilter: "unknown" }, { rangePreset: "unknown" }, { startTime: {} },
    { rangePreset: "custom", startTime: "invalid" }, { rangePreset: "custom", endTime: "invalid" },
    { rangePreset: "custom", startTime: "2026-09-14", endTime: "2026-09-13" }]) {
    assert.throws(() => buildEventFilterParams({ ...filters, ...edit }));
  }
  for (const limit of [0, -1, 501, NaN, Infinity, 1.5]) assert.throws(() => buildEventFilterParams(filters, limit));
  assert.throws(() => buildEventFilterParams(filters, undefined, "html"));
});

test("custom timestamp boundaries are inclusive and malformed event times never pass", () => {
  const input = { ...filters, rangePreset: "custom", startTime: row.timestamp, endTime: row.timestamp };
  assert.equal(matchesEventFilters(row, input), true);
  assert.equal(matchesEventFilters({ ...row, timestamp: "invalid" }, filters), false);
  assert.equal(matchesEventFilters({ ...row, timestamp: "2026-09-13T00:00:00.001Z" }, input), false);
  assert.equal(matchesEventFilters(row, { ...filters, categoryFilter: "platform" }), false);
  assert.equal(matchesEventFilters(row, { ...filters, severityFilter: "success" }), false);
});

test("relative windows expire a record immediately after the exact cutoff", () => {
  const now = Date.parse(row.timestamp) + 600000, input = { ...filters, rangePreset: "10m" };
  assert.equal(matchesEventFilters(row, input, now), true); assert.equal(matchesEventFilters(row, input, now + 1), false);
});

test("event mutations require exact confirmation and nonnegative safe-integer counts", () => {
  assert.equal(parseEventDeletion({ ok: true, deleted: 0 }), 0); assert.equal(parseUnread({ unread: 3 }), 3);
  assert.equal(parseReadAcknowledgement({ ok: true }), undefined);
  for (const value of [undefined, -1, "3", 0.1, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => parseUnread({ unread: value })); assert.throws(() => parseEventDeletion({ ok: true, deleted: value }));
  }
  for (const value of [null, [], {}, { ok: "true" }, { ok: false }]) {
    assert.throws(() => parseReadAcknowledgement(value)); assert.throws(() => parseEventDeletion(value));
  }
});

test("event transport keeps session credentials and private nonredirecting GET/POST semantics", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (url, init) => { calls.push({ url, ...init }); return Response.json({ ok: true }); });
  for (const method of ["GET", "POST"]) await requestEvent("https://engine.invalid/events", signal(), response => response.json(), method);
  assert.equal(calls.length, 2);
  for (const request of calls) {
    assert.equal(request.credentials, "include"); assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error");
    assert.equal(request.body, undefined); assert.ok(request.signal instanceof AbortSignal);
  }
});

test("pre-cancelled event requests do not dispatch", async t => {
  const fetch = t.mock.method(globalThis, "fetch", () => { throw new Error("must not dispatch"); });
  const parent = new AbortController(); parent.abort();
  await assert.rejects(requestEvent("https://engine.invalid/events", parent.signal, response => response.json()), { name: "AbortError" });
  assert.equal(fetch.mock.callCount(), 0);
});

test("event request deadline includes body decoding and releases its cancellation linkage", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let request;
  const parent = new AbortController(), remove = t.mock.method(parent.signal, "removeEventListener");
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return { ok: true, json: () => new Promise((_resolve, reject) => {
    init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
  }) }; });
  const pending = requestEvent("https://engine.invalid/events/delete", parent.signal, response => response.json(), "POST");
  await Promise.resolve(); t.mock.timers.tick(EVENT_REQUEST_TIMEOUT_MS - 1); assert.equal(request.signal.aborted, false);
  t.mock.timers.tick(1); await assert.rejects(pending, { name: "TimeoutError" }); assert.equal(remove.mock.callCount(), 1);
});

test("a cancelled late event response never reaches its decoder", async t => {
  let resolve, request, decoded = false;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return new Promise(done => { resolve = done; }); });
  const parent = new AbortController(), pending = requestEvent("https://engine.invalid/events/export", parent.signal, response => { decoded = true; return response.blob(); });
  parent.abort(); resolve(new Response("fixture")); await assert.rejects(pending, { name: "AbortError" });
  assert.equal(request.signal.aborted, true); assert.equal(decoded, false);
});

test("successful event reads release timers and parent listeners", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let request;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return Response.json({ unread: 0 }); });
  const parent = new AbortController(); await requestEvent("https://engine.invalid/events/unread-count", parent.signal,
    response => response.json(), "GET", EVENT_UNREAD_TIMEOUT_MS);
  parent.abort(); t.mock.timers.tick(EVENT_REQUEST_TIMEOUT_MS); assert.equal(request.signal.aborted, false);
});

test("HTTP, network and malformed JSON failures never retry event mutations", async t => {
  let calls = 0;
  for (const status of [400, 401, 403, 429, 500, 503]) {
    t.mock.method(globalThis, "fetch", () => { calls++; return Response.json({ ok: true }, { status }); });
    await assert.rejects(requestEvent("https://engine.invalid/events/delete", signal(), response => response.json(), "POST"),
      error => error instanceof EventHttpError && error.status === status);
  }
  t.mock.method(globalThis, "fetch", () => { calls++; throw new TypeError("offline"); });
  await assert.rejects(requestEvent("https://engine.invalid/events/delete", signal(), response => response.json(), "POST"), /offline/);
  t.mock.method(globalThis, "fetch", () => { calls++; return new Response("{broken"); });
  await assert.rejects(requestEvent("https://engine.invalid/events/delete", signal(), response => response.json(), "POST"), SyntaxError);
  assert.equal(calls, 8);
});

test("export decoding checks MIME type and preserves bytes with a safe format-bound filename", async () => {
  const body = "fixture,value\n", response = new Response(body, { headers: { "content-type": "text/csv; charset=utf-8", "content-disposition": 'attachment; filename="cyanrex-events-20260913.csv"' } });
  const result = await decodeEventExport(response, "csv"); assert.equal(result.filename, "cyanrex-events-20260913.csv");
  assert.equal(await result.blob.text(), body);
  const fallback = await decodeEventExport(new Response("[]", { headers: { "content-type": "application/json", "content-disposition": 'attachment; filename="../../other.exe"' } }), "json");
  assert.equal(fallback.filename, "cyanrex-events.json");
  await assert.rejects(decodeEventExport(new Response("<html>login</html>", { headers: { "content-type": "text/html" } }), "json"));
});
