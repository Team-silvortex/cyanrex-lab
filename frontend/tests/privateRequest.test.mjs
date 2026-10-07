import assert from "node:assert/strict";
import test from "node:test";
import { requestPrivate, requestPrivateJson } from "../src/transport/privateRequest.ts";
import { privateTransportModule } from "./helpers/privateTransportModules.mjs";

const { requestSettings } = await privateTransportModule("settings");
const { requestEvent, EventHttpError, decodeEventExport } = await privateTransportModule("events");
const { requestRuntimeJson, RuntimeHttpError } = await privateTransportModule("runtime");
const url = "https://engine.invalid/fixture";
const signal = () => new AbortController().signal;
const options = { timeoutMs: 1234, timeoutMessage: "Fixture deadline" };
const flush = async () => { for (let turn = 0; turn < 16; turn++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test("one private request passes the same response and owned signal to its decoder", async t => {
  const response = Response.json({ fixture: true });
  let request, decodes = 0;
  const fetch = t.mock.method(globalThis, "fetch", (target, init) => { request = { target, ...init }; return response; });
  const parent = new AbortController();
  assert.equal(await requestPrivate(url, parent.signal, async (actual, owned) => {
    decodes++; assert.equal(actual, response); assert.equal(owned, request.signal);
    assert.notEqual(owned, parent.signal); return "decoded";
  }, { ...options, body: { fixture: true }, credentials: "omit", cache: "force-cache", redirect: "follow" }), "decoded");
  assert.equal(fetch.mock.callCount(), 1); assert.equal(decodes, 1); assert.equal(request.target, url);
  assert.equal(request.method, "POST"); assert.deepEqual(JSON.parse(request.body), { fixture: true });
  assert.deepEqual(request.headers, { "Content-Type": "application/json" });
  assert.equal(request.credentials, "include"); assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error");
});

test("body-free reads and explicit body-free POSTs add neither JSON nor authority headers", async t => {
  const requests = [];
  t.mock.method(globalThis, "fetch", (_url, init) => { requests.push(init); return new Response("fixture"); });
  await requestPrivate(url, signal(), response => response.text(), options);
  await requestPrivate(url, signal(), response => response.text(), { ...options, method: "POST" });
  assert.deepEqual(requests.map(request => request.method), ["GET", "POST"]);
  for (const request of requests) { assert.equal(request.headers, undefined); assert.equal(request.body, undefined); }
});

test("pre-cancellation preserves the exact parent reason and dispatches nothing", async t => {
  const fetch = t.mock.method(globalThis, "fetch", () => { throw new Error("must not dispatch"); });
  const parent = new AbortController(), reason = new Error("navigation replaced owner"); parent.abort(reason);
  await assert.rejects(requestPrivate(url, parent.signal, async () => "unused", options), error => error === reason);
  assert.equal(fetch.mock.callCount(), 0);
});

test("header deadline settles without abort cooperation and late headers never enter the decoder", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const held = deferred(); let request, outcome, decoded = 0;
  const fetch = t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return held.promise; });
  const completed = requestPrivate(url, signal(), async () => { decoded++; return "late"; }, options)
    .then(value => { outcome = { value }; }, error => { outcome = { error }; });
  t.mock.timers.tick(options.timeoutMs - 1); await flush();
  assert.equal(request.signal.aborted, false); assert.equal(outcome, undefined);
  t.mock.timers.tick(1); await flush(); const bounded = outcome;
  held.resolve(Response.json({ late: true })); await completed; await flush();
  assert.equal(bounded?.error?.name, "TimeoutError"); assert.equal(bounded.error.message, options.timeoutMessage);
  assert.equal(request.signal.aborted, true); assert.equal(decoded, 0); assert.equal(fetch.mock.callCount(), 1);
});

test("the same deadline includes an abort-ignoring response decoder", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const held = deferred(); let entered = false, outcome;
  t.mock.method(globalThis, "fetch", () => Response.json({ fixture: true }));
  const completed = requestPrivate(url, signal(), async () => { entered = true; return held.promise; }, options)
    .then(value => { outcome = { value }; }, error => { outcome = { error }; });
  await flush(); assert.equal(entered, true);
  t.mock.timers.tick(options.timeoutMs); await flush(); const bounded = outcome;
  held.resolve("late parsed result"); await completed; await flush();
  assert.equal(bounded?.error?.name, "TimeoutError"); assert.equal(outcome, bounded);
});

test("parent cancellation ends header and body waits immediately with the original reason", async t => {
  const results = [];
  for (const phase of ["headers", "body"]) {
    const held = deferred(), parent = new AbortController(), reason = { fixture: phase };
    let request, outcome;
    t.mock.method(globalThis, "fetch", (_url, init) => { request = init;
      return phase === "headers" ? held.promise : { json: () => held.promise }; });
    const completed = requestPrivate(url, parent.signal, response => response.json(), options)
      .then(value => { outcome = { value }; }, error => { outcome = { error }; });
    await flush(); parent.abort(reason); await flush(); const cancelled = outcome;
    held.resolve(phase === "headers" ? Response.json({ late: true }) : { late: true }); await completed;
    results.push({ cancelled, reason, ownedReason: request.signal.reason });
  }
  for (const result of results) {
    assert.equal(result.cancelled?.error, result.reason); assert.equal(result.ownedReason, result.reason);
  }
});

test("completed and rejected work release the timer and both cancellation listeners", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const fail of [false, true]) {
    const parent = new AbortController(), removedParent = t.mock.method(parent.signal, "removeEventListener");
    let request, removedOwned;
    t.mock.method(globalThis, "fetch", (_url, init) => {
      request = init; removedOwned = t.mock.method(init.signal, "removeEventListener"); return Response.json({});
    });
    const rejected = new Error("decoder refused record");
    const result = requestPrivate(url, parent.signal, async () => { if (fail) throw rejected; return "accepted"; }, options);
    if (fail) await assert.rejects(result, error => error === rejected); else assert.equal(await result, "accepted");
    assert.equal(removedParent.mock.callCount(), 1); assert.equal(removedOwned.mock.callCount(), 1);
    parent.abort(); t.mock.timers.tick(options.timeoutMs * 2); assert.equal(request.signal.aborted, false);
  }
});

test("late fetch and body rejections after a deadline are observed without unhandled rejection", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const unhandled = [], onUnhandled = reason => unhandled.push(reason);
  process.on("unhandledRejection", onUnhandled);
  try {
    for (const phase of ["headers", "body"]) {
      const held = deferred(); let outcome;
      t.mock.method(globalThis, "fetch", () => phase === "headers" ? held.promise : { json: () => held.promise });
      const completed = requestPrivate(url, signal(), response => response.json(), options)
        .then(value => { outcome = { value }; }, error => { outcome = { error }; });
      await flush(); t.mock.timers.tick(options.timeoutMs); await flush(); const bounded = outcome;
      held.reject(new Error(`late ${phase} failure`)); await completed;
      await new Promise(resolve => setImmediate(resolve));
      assert.equal(bounded?.error?.name, "TimeoutError"); assert.equal(outcome, bounded);
    }
    assert.deepEqual(unhandled, []);
  } finally { process.removeListener("unhandledRejection", onUnhandled); }
});

test("network and decoder failures propagate without any automatic retry", async t => {
  const network = new TypeError("offline"), invalid = new Error("invalid fixture record");
  const fetch = t.mock.method(globalThis, "fetch", () => { throw network; });
  await assert.rejects(requestPrivate(url, signal(), async () => "unused", options), error => error === network);
  fetch.mock.mockImplementation(() => Response.json({}));
  await assert.rejects(requestPrivate(url, signal(), async () => { throw invalid; }, options), error => error === invalid);
  assert.equal(fetch.mock.callCount(), 2);
});

test("the primitive leaves HTTP status and response representation to its selected decoder", async t => {
  t.mock.method(globalThis, "fetch", () => new Response("fixture detail", { status: 503, headers: { "Content-Type": "text/plain" } }));
  assert.deepEqual(await requestPrivate(url, signal(), async response => ({ status: response.status, text: await response.text() }), options),
    { status: 503, text: "fixture detail" });
});

test("JSON convenience keeps read and write deadlines at ten and twenty seconds", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const [body, timeout, method] of [[undefined, 10_000, "GET"], [{ fixture: true }, 20_000, "POST"]]) {
    const held = deferred(); let request, outcome;
    t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return held.promise; });
    const completed = requestPrivateJson(url, signal(), value => value, body)
      .then(value => { outcome = { value }; }, error => { outcome = { error }; });
    t.mock.timers.tick(timeout - 1); await flush(); assert.equal(outcome, undefined); assert.equal(request.signal.aborted, false);
    t.mock.timers.tick(1); await flush(); const bounded = outcome;
    held.resolve(Response.json({})); await completed; await flush();
    assert.equal(request.method, method); assert.equal(bounded?.error?.name, "TimeoutError");
    assert.equal(bounded.error.message, "Engine request timed out");
  }
});

test("JSON deadline and parent cancellation never pass late values to the value decoder", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const mode of ["timeout", "parent"]) {
    const held = deferred(), parent = new AbortController(), reason = new Error("new owner");
    let outcome, decoded = 0;
    t.mock.method(globalThis, "fetch", () => ({ ok: true, headers: new Headers({ "Content-Type": "application/json" }), json: () => held.promise }));
    const completed = requestPrivateJson(url, parent.signal, value => { decoded++; return value; }, undefined, 23, "Fixture")
      .then(value => { outcome = { value }; }, error => { outcome = { error }; });
    await flush(); if (mode === "timeout") t.mock.timers.tick(23); else parent.abort(reason);
    await flush(); const bounded = outcome; held.resolve({ late: true }); await completed; await flush();
    assert.equal(decoded, 0);
    if (mode === "timeout") assert.equal(bounded?.error?.name, "TimeoutError"); else assert.equal(bounded?.error, reason);
  }
});

test("JSON convenience checks successful status and JSON media type before decoding", async t => {
  let decoded = 0, reads = 0;
  for (const [ok, type] of [[false, "application/json"], [true, "text/html"], [true, null]]) {
    t.mock.method(globalThis, "fetch", () => ({ ok, headers: new Headers(type ? { "Content-Type": type } : {}),
      json: async () => { reads++; return {}; }, body: { cancel: async () => undefined } }));
    await assert.rejects(requestPrivateJson(url, signal(), () => { decoded++; }, undefined, 100, "Fixture"), { message: "Fixture request failed" });
  }
  assert.equal(decoded, 0); assert.equal(reads, 0);
  t.mock.method(globalThis, "fetch", () => new Response('{"fixture":true}', { headers: { "Content-Type": "Application/JSON; charset=utf-8" } }));
  assert.deepEqual(await requestPrivateJson(url, signal(), value => value), { fixture: true });
});

test("malformed JSON stays a parsing failure and cannot become an acknowledgement", async t => {
  let decoded = 0;
  const fetch = t.mock.method(globalThis, "fetch", () => new Response("{bad", { headers: { "Content-Type": "application/json" } }));
  await assert.rejects(requestPrivateJson(url, signal(), () => { decoded++; }, { fixture: true }), SyntaxError);
  assert.equal(decoded, 0); assert.equal(fetch.mock.callCount(), 1);
});

test("feature adapters retain distinct HTTP errors after sharing the same transport", async t => {
  const payload = { message: "synthetic runtime detail", fixture: true };
  let bodyReads = 0;
  const fetch = t.mock.method(globalThis, "fetch", () => ({ ok: false, status: 403,
    headers: new Headers({ "Content-Type": "application/json" }), body: { cancel: async () => undefined },
    json: async () => { bodyReads++; return payload; } }));
  await assert.rejects(requestSettings(url, signal(), value => value), { name: "Error", message: "Settings request failed" });
  await assert.rejects(requestEvent(url, signal(), response => response.json()), error => error instanceof EventHttpError && error.status === 403);
  assert.equal(bodyReads, 0);
  await assert.rejects(requestRuntimeJson(url, signal()), error => error instanceof RuntimeHttpError
    && error.status === 403 && error.payload === payload && error.message === payload.message);
  assert.equal(bodyReads, 1); assert.equal(fetch.mock.callCount(), 3);
});

test("shared transport preserves event Blob exports without imposing the JSON response policy", async t => {
  const text = "fixture,value\n";
  t.mock.method(globalThis, "fetch", () => new Response(text, { headers: {
    "Content-Type": "text/csv; charset=utf-8", "Content-Disposition": 'attachment; filename="fixture.csv"',
  } }));
  const result = await requestEvent(url, signal(), response => decodeEventExport(response, "csv"));
  assert.equal(result.filename, "fixture.csv"); assert.equal(await result.blob.text(), text);
});
