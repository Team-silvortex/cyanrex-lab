import assert from "node:assert/strict";
import { getEventListeners } from "node:events";
import test from "node:test";
import { platformRequest } from "../../engine/src/bin/cyanrex-platform/session_ui/request.js";

// Native Response/ReadableStream/AbortSignal behavior only. No server, browser, database,
// authentication, JSON business parsing or persisted command outcome is modeled here.
const SESSION = "/platform/v1/session", LOGIN = `${SESSION}/login`, LOGOUT = `${SESSION}/logout`;
const INDEX = "/platform/v1/task-index?limit=20";
const ID = "00000000-0000-4000-8000-000000000001", TASK = `/platform/v1/tasks/${ID}`;
const SECRET = "synthetic-private-value-never-exposed";
const bytes = value => new TextEncoder().encode(value);
const tick = () => new Promise(resolve => setImmediate(resolve));
const headers = { "content-type": "application/json" };

function reply(raw = "{}", status = 200, media = "application/json") {
  return new Response(raw, { status, headers: media === null ? {} : { "content-type": media } });
}
function nativeStream(parts, options = {}) {
  let index = 0, cancelled = 0, terminal = 'pending';
  const stream = new ReadableStream({
    pull(controller) {
      if (options.error) { controller.error(options.error); terminal = 'errored'; }
      else if (index < parts.length) controller.enqueue(parts[index++]);
      else if (options.close !== false) { controller.close(); terminal = 'closed'; }
    },
    cancel() { cancelled++; terminal = 'cancelled'; return options.cancel?.(); },
  });
  // Response.clone owns a native tee reader; its closed source can remain locked.
  // Assert actual EOF/error/cancellation instead, with owned branch cleanup checked
  // separately in platformSessionRequestLifecycle.test.mjs (not merely a lock bit).
  return { stream, cancelled: () => cancelled, terminal: () => terminal };
}
function unknown(error, status) {
  assert.ok(error instanceof Error);
  assert.equal(error.message, "platformRequest.unknown");
  if (status === undefined) assert.equal(Object.hasOwn(error, "deniedStatus"), false);
  else assert.equal(error.deniedStatus, status);
  assert.ok(!JSON.stringify(error).includes(SECRET));
  assert.ok(!String(error.stack).includes(SECRET));
  return true;
}
function clock(t) {
  const timers = [], cleared = new Set();
  t.mock.method(globalThis, "setTimeout", (callback, delay) => {
    assert.equal(delay, 10_000, "the transport owns one ten-second browser waiting bound");
    const timer = { callback }; timers.push(timer); return timer;
  });
  t.mock.method(globalThis, "clearTimeout", timer => { cleared.add(timer); });
  return { fire() { assert.equal(timers.length, 1); timers[0].callback(); },
    clean() { assert.equal(timers.length, cleared.size); } };
}

test("only the existing canonical route/method set dispatches with private same-origin options", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (path, options) => {
    calls.push({ path, options }); return Promise.resolve(reply());
  });
  const selected = [[SESSION, "GET"], [LOGIN, "POST", "{}"], [LOGOUT, "POST"],
    [INDEX, "GET"], [`${INDEX}&after=${ID}`, "GET"], [TASK, "GET"], [`${TASK}/content`, "PUT", "{}"]];
  for (const [path, method, body] of selected) {
    assert.deepEqual(await platformRequest(path, method, body === undefined ? {} : { body }), { status: 200, raw: "{}" });
  }
  assert.equal(calls.length, selected.length);
  for (const [index, { path, options }] of calls.entries()) {
    const [expectedPath, method, body] = selected[index];
    assert.equal(path, expectedPath); assert.equal(options.method, method);
    assert.equal(options.mode, "same-origin"); assert.equal(options.credentials, "same-origin");
    assert.equal(options.cache, "no-store"); assert.equal(options.redirect, "error");
    assert.ok(options.signal instanceof AbortSignal);
    const sent = new Headers(options.headers);
    assert.equal(sent.get("x-cyanrex-platform-request"), "1");
    assert.equal(sent.get("content-type"), method === "GET" ? null : "application/json");
    assert.equal(sent.get("authorization"), null); assert.equal(sent.get("cookie"), null);
    assert.deepEqual([...sent.keys()].sort(), method === "GET"
      ? ["x-cyanrex-platform-request"] : ["content-type", "x-cyanrex-platform-request"]);
    assert.equal(options.body, body);
    assert.equal(getEventListeners(options.signal, "abort").length, 0);
  }
});

test("absolute, ambiguous, unmounted and wrong-method targets fail before fetch or handoff", async t => {
  let calls = 0, handoffs = 0;
  t.mock.method(globalThis, "fetch", () => { calls++; return Promise.resolve(reply()); });
  const nil = "00000000-0000-0000-0000-000000000000";
  const targets = [[`https://example.invalid${SESSION}`, "GET"], [`//example.invalid${SESSION}`, "GET"],
    [SESSION.slice(1), "GET"], [`${SESSION}/`, "GET"], [`${SESSION}?`, "GET"], [`${SESSION}#fragment`, "GET"],
    ["/platform/v1/./session", "GET"], ["/platform/v1/%73ession", "GET"], ["/platform\\v1/session", "GET"],
    [`${SESSION}\n`, "GET"], [SESSION, "get"], [SESSION, "POST"], [LOGIN, "GET"], [LOGOUT, "GET"],
    ["/platform/v1/task-index", "GET"], ["/platform/v1/task-index?limit=020", "GET"],
    ["/platform/v1/task-index?after=" + ID + "&limit=20", "GET"], [`${INDEX}&limit=20`, "GET"],
    [`${INDEX}&after=${ID}&after=${ID}`, "GET"], [`${INDEX}&after=${nil}`, "GET"],
    [`${INDEX}&after=${ID.toUpperCase().replace(/1$/, "A")}`, "GET"], [`${INDEX}&after=%30${ID.slice(1)}`, "GET"],
    [`${INDEX}&after=${ID}&extra=1`, "GET"], [`${INDEX}&after=${ID}#x`, "GET"],
    [`${INDEX}\n`, "GET"], [`${INDEX}&after=${ID}\n`, "GET"],
    [`/platform/v1/tasks/${nil}`, "GET"], [`${TASK}/`, "GET"], [`${TASK}?extra=1`, "GET"],
    [`${TASK}\n`, "GET"], [`${TASK}\r`, "GET"],
    [`${TASK}/content`, "GET"], [TASK, "PUT"], [TASK, "DELETE"], [`${TASK}/content?`, "PUT"],
    ["/platform/v1/text-artifacts", "POST"], [`${TASK}/text-artifacts`, "PUT"],
    [new URL(`https://example.invalid${SESSION}`), "GET"], [null, "GET"], [SESSION, null]];
  for (const [path, method] of targets) {
    const body = method === "GET" || path === LOGOUT ? undefined : "{}";
    await assert.rejects(platformRequest(path, method, { body, onHandedOff() { handoffs++; } }), unknown);
  }
  assert.equal(calls, 0); assert.equal(handoffs, 0);
});

test("body admission uses route-selected UTF-8 bounds and preserves bodyless logout", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (path, options) => { calls.push({ path, options }); return Promise.resolve(reply()); });
  const invalid = [[SESSION, "GET", "{}"], [INDEX, "GET", ""], [TASK, "GET", "{}"],
    [LOGIN, "POST", undefined], [LOGIN, "POST", {}], [LOGIN, "POST", "é".repeat(16 * 1024) + "a"],
    [LOGOUT, "POST", ""], [LOGOUT, "POST", "{}"], [`${TASK}/content`, "PUT", undefined],
    [`${TASK}/content`, "PUT", {}], [`${TASK}/content`, "PUT", "é".repeat(32 * 1024) + "a"]];
  for (const [path, method, body] of invalid) await assert.rejects(platformRequest(path, method, { body }), unknown);
  assert.equal(calls.length, 0);
  await platformRequest(LOGIN, "POST", { body: "é".repeat(16 * 1024) });
  await platformRequest(`${TASK}/content`, "PUT", { body: "é".repeat(32 * 1024) });
  await platformRequest(LOGOUT, "POST");
  assert.equal(calls.length, 3); assert.equal(calls.at(-1).options.body, undefined);
});

test("unknown, accessor, inherited and capability-bearing options are rejected without execution", async t => {
  let calls = 0, getters = 0;
  t.mock.method(globalThis, "fetch", () => { calls++; return Promise.resolve(reply()); });
  const accessor = Object.defineProperty({}, "body", { enumerable: true, get() { getters++; return "{}"; } });
  const symbol = { [Symbol("private")]: SECRET };
  const invalid = [null, [], Object.create({ body: "{}" }), accessor, symbol,
    { headers: { Authorization: SECRET } }, { cookie: SECRET }, { timeout: 1 }, { extra: true },
    { signal: { aborted: false, addEventListener() {} } }, { signal: null },
    { onHandedOff: true }, { onDenied: "callback" }];
  for (const options of invalid) await assert.rejects(platformRequest(LOGIN, "POST", options), unknown);
  assert.equal(calls, 0); assert.equal(getters, 0);
});

test("signal admission rejects forged brands, own capability shadows and intermediate prototypes before access", async t => {
  let calls = 0, getters = 0;
  t.mock.method(globalThis, "fetch", async () => { calls++; return reply(); });
  const forged = Object.create(AbortSignal.prototype), signals = [["forged native prototype", forged]];
  for (const key of ["aborted", "addEventListener", "removeEventListener"]) {
    const signal = new AbortController().signal;
    Object.defineProperty(signal, key, { get() { getters++; throw new Error(SECRET); } });
    signals.push([`own ${key} accessor`, signal]);
  }
  const custom = new AbortController().signal;
  Object.setPrototypeOf(custom, Object.create(AbortSignal.prototype));
  signals.push(["custom intermediate prototype", custom]);
  const observations = [];
  for (const [name, signal] of signals) {
    const beforeCalls = calls, beforeGetters = getters;
    let failure;
    try { await platformRequest(SESSION, "GET", { signal }); } catch (error) { failure = error; }
    observations.push({ name, fixedError: failure instanceof Error && failure.message === "platformRequest.unknown",
      calledFetch: calls - beforeCalls, readAccessor: getters - beforeGetters,
      disclosedReason: String(failure?.stack).includes(SECRET) });
  }
  assert.deepEqual(observations, signals.map(([name]) => ({ name, fixedError: true,
    calledFetch: 0, readAccessor: 0, disclosedReason: false })));
});

test("handoff occurs exactly once immediately after fetch invocation, even for synchronous throw", async t => {
  const order = [], pending = Promise.withResolvers();
  t.mock.method(globalThis, "fetch", () => { order.push("fetch"); return pending.promise; });
  const waiting = platformRequest(LOGOUT, "POST", { onHandedOff() { order.push("handoff"); } });
  assert.deepEqual(order, ["fetch", "handoff"]);
  pending.resolve(reply()); await waiting;
  assert.deepEqual(order, ["fetch", "handoff"]);
  t.mock.method(globalThis, "fetch", () => { order.push("throw"); throw new Error(SECRET); });
  await assert.rejects(platformRequest(LOGOUT, "POST", { onHandedOff() { order.push("throw-handoff"); } }), unknown);
  assert.deepEqual(order, ["fetch", "handoff", "throw", "throw-handoff"]);
});

test("canonical MIME and fatal UTF-8 retain raw text for caller-owned strict JSON parsing", async t => {
  const expected = '{"text":"你好😀"}';
  for (const media of ["application/json", "application/json; charset=utf-8", "Application/JSON; CHARSET=UTF-8"]) {
    const encoded = bytes(expected), input = nativeStream([...encoded].map(value => Uint8Array.of(value)));
    t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { headers: { "content-type": media } }));
    assert.deepEqual(await platformRequest(SESSION, "GET"), { status: 200, raw: expected });
    assert.equal(input.terminal(), 'closed'); assert.equal(input.cancelled(), 0);
  }
  t.mock.method(globalThis, "fetch", async () => reply("not business JSON", 503));
  assert.deepEqual(await platformRequest(SESSION, "GET"), { status: 503, raw: "not business JSON" });
});

test("invalid MIME, missing body and invalid UTF-8 never produce a usable response", async t => {
  for (const media of [null, "text/html", "application/json;charset=latin1", "application/json; extra=value", "application/json, text/html"]) {
    const input = nativeStream([bytes("{}")], { close: false });
    t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { headers: media ? { "content-type": media } : {} }));
    await assert.rejects(platformRequest(SESSION, "GET"), unknown);
    assert.equal(input.cancelled(), 1); assert.equal(input.stream.locked, false);
  }
  t.mock.method(globalThis, "fetch", async () => new Response(null, { headers }));
  await assert.rejects(platformRequest(SESSION, "GET"), unknown);
  const input = nativeStream([Uint8Array.of(0xc3, 0x28)]);
  t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { headers }));
  await assert.rejects(platformRequest(SESSION, "GET"), unknown);
  assert.equal(input.terminal(), 'closed'); assert.equal(input.cancelled(), 0);
});

test("actual collected bytes use each fixed route budget, independently of advertised length", async t => {
  const routes = [[SESSION, "GET", 4096], [LOGIN, "POST", 4096, "{}"], [LOGOUT, "POST", 4096],
    [INDEX, "GET", 128 * 1024], [TASK, "GET", 50 * 1024 * 1024], [`${TASK}/content`, "PUT", 128 * 1024, "{}"]];
  for (const [path, method, limit, body] of routes) {
    const input = nativeStream([new Uint8Array(limit + 1)], { close: false });
    t.mock.method(globalThis, "fetch", async () => new Response(input.stream,
      { headers: { ...headers, "content-length": "1" } }));
    await assert.rejects(platformRequest(path, method, { body }), unknown);
    assert.equal(input.cancelled(), 1); assert.equal(input.terminal(), 'cancelled');
  }
  for (const [path, method, limit, body] of routes.filter(([, , limit]) => limit < 50 * 1024 * 1024)) {
    t.mock.method(globalThis, "fetch", async () => reply("é".repeat(limit / 2)));
    assert.equal(bytes((await platformRequest(path, method, { body })).raw).length, limit);
  }
});

test("empty ready frames consume the per-route frame budget without starving indefinitely", async t => {
  for (const [path, method, limit, body] of [[SESSION, "GET", 4096], [INDEX, "GET", 16_384],
    [TASK, "GET", 16_384], [`${TASK}/content`, "PUT", 4096, "{}"]]) {
    let frames = 0, cancelled = 0, terminal = 'pending';
    const stream = new ReadableStream({ pull(controller) {
      if (++frames <= limit + 1) controller.enqueue(new Uint8Array());
    }, cancel() { cancelled++; terminal = 'cancelled'; } });
    t.mock.method(globalThis, "fetch", async () => new Response(stream, { headers }));
    await assert.rejects(platformRequest(path, method, { body }), unknown);
    assert.equal(cancelled, 1); assert.equal(terminal, 'cancelled');
  }
  const input = nativeStream(Array.from({ length: 4096 }, () => new Uint8Array()));
  t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { headers }));
  assert.deepEqual(await platformRequest(SESSION, "GET"), { status: 200, raw: "" });
});

test("denial callback runs from headers before body lock/read and only for 401 or 403", async t => {
  for (const status of [401, 403, 404, 409, 503]) {
    const order = [], pending = Promise.withResolvers();
    const stream = new ReadableStream({ start(controller) { pending.resolve(controller); } });
    const controller = await pending.promise;
    t.mock.method(globalThis, "fetch", async () => new Response(stream, { status, headers }));
    const waiting = platformRequest(`${TASK}/content`, "PUT", { body: "{}", onDenied(value) {
      order.push(value); assert.equal(stream.locked, false, "deny UI invalidates before body collection");
    } });
    await tick();
    assert.deepEqual(order, status === 401 || status === 403 ? [status] : []);
    controller.enqueue(bytes("{}")); controller.close();
    assert.deepEqual(await waiting, { status, raw: "{}" });
  }
});

test("malformed denial bodies preserve only known status and never leak underlying errors", async t => {
  for (const status of [401, 403, 409, 503]) {
    const seen = [];
    t.mock.method(globalThis, "fetch", async () => reply(SECRET, status, "text/html"));
    await assert.rejects(platformRequest(SESSION, "GET", { onDenied(value) { seen.push(value); } }),
      error => unknown(error, status === 401 || status === 403 ? status : undefined));
    assert.deepEqual(seen, status === 401 || status === 403 ? [status] : []);
  }
});

test("pre-aborted signals never dispatch; later cancellation settles even if fetch ignores it", async t => {
  const before = new AbortController(), parent = new AbortController();
  before.abort(new Error(SECRET));
  let calls = 0, handoffs = 0, denied = 0;
  const pending = Promise.withResolvers();
  t.mock.method(globalThis, "fetch", () => { calls++; return pending.promise; });
  await assert.rejects(platformRequest(SESSION, "GET", { signal: before.signal, onHandedOff() { handoffs++; } }), unknown);
  assert.equal(calls, 0); assert.equal(handoffs, 0);
  const waiting = platformRequest(SESSION, "GET", { signal: parent.signal, onHandedOff() { handoffs++; }, onDenied() { denied++; } });
  parent.abort(new Error(SECRET)); await assert.rejects(waiting, unknown);
  assert.equal(calls, 1); assert.equal(handoffs, 1);
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  const late = nativeStream([bytes("{}")], { close: false });
  pending.resolve(new Response(late.stream, { status: 401, headers })); await tick();
  assert.equal(late.cancelled(), 1); assert.equal(denied, 0, "late denial cannot affect a new UI epoch");
  assert.equal(calls, 1, "cancellation never retries");
});

test("ten-second waiting timeout releases listeners/timer and cancels a late response without retry", async t => {
  const timer = clock(t), parent = new AbortController(), pending = Promise.withResolvers();
  let calls = 0, handed = 0;
  t.mock.method(globalThis, "fetch", () => { calls++; return pending.promise; });
  const waiting = platformRequest(LOGIN, "POST", { body: "{}", signal: parent.signal, onHandedOff() { handed++; } });
  timer.fire(); await assert.rejects(waiting, unknown);
  timer.clean(); assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  const late = nativeStream([bytes("{}")], { close: false });
  pending.resolve(new Response(late.stream, { headers })); await tick();
  assert.equal(late.cancelled(), 1); assert.equal(calls, 1); assert.equal(handed, 1);
});

test("body cancellation releases its reader and retains denial metadata after the waiting bound", async t => {
  const timer = clock(t), parent = new AbortController(), input = nativeStream([], { close: false });
  let signal, denied = 0;
  t.mock.method(globalThis, "fetch", async (_path, options) => {
    signal = options.signal; return new Response(input.stream, { status: 403, headers });
  });
  const waiting = platformRequest(`${TASK}/content`, "PUT", { body: "{}", signal: parent.signal,
    onDenied(status) { assert.equal(status, 403); denied++; } });
  await tick(); assert.equal(input.stream.locked, true); assert.equal(denied, 1);
  timer.fire(); await assert.rejects(waiting, error => unknown(error, 403)); await tick();
  assert.equal(input.cancelled(), 1); assert.equal(input.terminal(), 'cancelled');
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  assert.equal(getEventListeners(signal, "abort").length, 0); timer.clean();
});

test("success and rejected fetch clean up one wait and expose no upstream reason", async t => {
  const timer = clock(t), parent = new AbortController();
  let signal;
  t.mock.method(globalThis, "fetch", async (_path, options) => { signal = options.signal; return reply(); });
  await platformRequest(SESSION, "GET", { signal: parent.signal }); timer.clean();
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  assert.equal(getEventListeners(signal, "abort").length, 0);
  t.mock.method(globalThis, "fetch", () => Promise.reject(new Error(SECRET)));
  await assert.rejects(platformRequest(SESSION, "GET", { signal: parent.signal }), unknown); timer.clean();
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
});

test("stream read failure and rejecting cancellation reach an actual source terminal state", async t => {
  const failed = nativeStream([], { error: new Error(SECRET) });
  t.mock.method(globalThis, "fetch", async () => new Response(failed.stream, { headers }));
  await assert.rejects(platformRequest(SESSION, "GET"), unknown); assert.equal(failed.terminal(), 'errored');
  const input = nativeStream([new Uint8Array(4097)], { close: false,
    cancel() { return Promise.reject(new Error(SECRET)); } });
  t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { headers }));
  await assert.rejects(platformRequest(SESSION, "GET"), unknown); await tick();
  assert.equal(input.cancelled(), 1); assert.equal(input.terminal(), 'cancelled');
});

test("a throwing handoff hook stops waiting and consumes later transport settlement safely", async t => {
  const pending = Promise.withResolvers();
  let handoffs = 0, denied = 0;
  t.mock.method(globalThis, "fetch", () => pending.promise);
  await assert.rejects(platformRequest(LOGOUT, "POST", { onHandedOff() { handoffs++; throw new Error(SECRET); },
    onDenied() { denied++; } }), unknown);
  const late = nativeStream([bytes("{}")], { close: false });
  pending.resolve(new Response(late.stream, { status: 401, headers })); await tick();
  assert.equal(late.cancelled(), 1); assert.equal(handoffs, 1); assert.equal(denied, 0);
  t.mock.method(globalThis, "fetch", () => Promise.reject(new Error(SECRET)));
  await assert.rejects(platformRequest(LOGOUT, "POST", { onHandedOff() { throw new Error(SECRET); } }), unknown);
  await tick(); // A discarded rejected fetch must not escape as an unhandled rejection.
});

test("a throwing denial hook retains its known status and cancels before acquiring a body reader", async t => {
  const input = nativeStream([bytes("{}")], { close: false });
  t.mock.method(globalThis, "fetch", async () => new Response(input.stream, { status: 401, headers }));
  await assert.rejects(platformRequest(`${TASK}/content`, "PUT", { body: "{}",
    onDenied() { assert.equal(input.stream.locked, false); throw new Error(SECRET); } }), error => unknown(error, 401));
  assert.equal(input.cancelled(), 1); assert.equal(input.stream.locked, false);
  assert.equal(input.terminal(), 'cancelled');
});

test("external cancellation during body reading releases reader and both signal listeners", async t => {
  const parent = new AbortController(), input = nativeStream([], { close: false });
  let signal;
  t.mock.method(globalThis, "fetch", async (_path, options) => { signal = options.signal; return new Response(input.stream, { headers }); });
  const waiting = platformRequest(TASK, "GET", { signal: parent.signal });
  await tick(); assert.equal(input.stream.locked, true);
  parent.abort(new Error(SECRET)); await assert.rejects(waiting, unknown); await tick();
  assert.equal(input.cancelled(), 1); assert.equal(input.terminal(), 'cancelled');
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  assert.equal(getEventListeners(signal, "abort").length, 0);
});
