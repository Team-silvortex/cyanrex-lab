import assert from "node:assert/strict";
import { getEventListeners } from "node:events";
import test from "node:test";
import { platformRequest } from "../../engine/src/bin/cyanrex-platform/session_ui/request.js";

// Native Response/ReadableStream plus method probes only. No HTTP/server saving or credentials.
const path = "/platform/v1/session", headers = { "content-type": "application/json" };
const secret = "synthetic-upstream-detail-must-not-escape";
const encode = value => new TextEncoder().encode(value);
const tick = () => new Promise(resolve => setImmediate(resolve));
function unknown(error) {
  assert.equal(error.message, "platformRequest.unknown");
  assert.ok(!String(error.stack).includes(secret)); return true;
}
function instrument(t, response, nativeBody) {
  const order = [], clone = response.clone.bind(response), native = response.arrayBuffer.bind(response);
  t.mock.method(response, "clone", () => {
    order.push("clone"); const copy = clone(), getReader = copy.body.getReader.bind(copy.body);
    t.mock.method(copy.body, "getReader", () => {
      const reader = getReader(), read = reader.read.bind(reader);
      t.mock.method(reader, "read", async () => {
        const item = await read(); if (item.done) order.push("guard-eof"); return item;
      });
      return reader;
    });
    return copy;
  });
  t.mock.method(response, "arrayBuffer", () => {
    order.push("native");
    assert.ok(order.includes("guard-eof"), "native consumption cannot race the bounded guard");
    return nativeBody ? nativeBody() : native();
  });
  return order;
}

test("native closed tee source can remain internally locked without more pulls or outstanding I/O", async () => {
  let terminal = 'pending', pulls = 0, cancellations = 0;
  const source = new ReadableStream({ pull(controller) {
    pulls++;
    controller.enqueue(encode('{}')); controller.close(); terminal = 'closed';
  }, cancel() { cancellations++; } });
  const original = new Response(source), guard = original.clone();
  assert.equal(await guard.text(), '{}'); assert.equal(await original.text(), '{}');
  assert.equal(terminal, 'closed'); assert.equal(source.locked, true);
  const settledPulls = pulls; await tick();
  assert.equal(pulls, settledPulls); assert.equal(cancellations, 0);
});

test("only guarded complete EOF permits exactly one native original-body consumption", async t => {
  const response = new Response('{"text":"合作 😀"}', { headers });
  const order = instrument(t, response); let calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls++; return response; });
  assert.deepEqual(await platformRequest(path, "GET"), { status: 200, raw: '{"text":"合作 😀"}' });
  assert.deepEqual(order, ["clone", "guard-eof", "native"]); assert.equal(calls, 1);
});

test("an incomplete guard cannot start native consumption even when headers and chunks are ready", async t => {
  let source;
  const response = new Response(new ReadableStream({ start(controller) { source = controller; } }), { headers });
  const order = instrument(t, response);
  t.mock.method(globalThis, "fetch", async () => response);
  const waiting = platformRequest(path, "GET"); await tick();
  assert.deepEqual(order, ["clone"]);
  source.enqueue(encode('{"ready":')); await tick(); assert.deepEqual(order, ["clone"]);
  source.enqueue(encode("true}")); source.close();
  assert.deepEqual(await waiting, { status: 200, raw: '{"ready":true}' });
  assert.deepEqual(order, ["clone", "guard-eof", "native"]);
});

test("guard byte overflow never invokes the unbounded native consumer and cancels the original source", async t => {
  let cancelled = 0, sent = false;
  const response = new Response(new ReadableStream({ pull(controller) {
    if (!sent) { sent = true; controller.enqueue(new Uint8Array(4097)); }
  }, cancel() { cancelled++; } }), { headers });
  const order = instrument(t, response); let calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls++; return response; });
  await assert.rejects(platformRequest(path, "GET"), unknown); await tick();
  assert.deepEqual(order, ["clone"]); assert.equal(calls, 1); assert.equal(cancelled, 1);
});

test("guard invalid UTF-8 rejects before original native consumption", async t => {
  const response = new Response(Uint8Array.of(0xc3, 0x28), { headers });
  const order = instrument(t, response);
  t.mock.method(globalThis, "fetch", async () => response);
  await assert.rejects(platformRequest(path, "GET"), unknown);
  assert.deepEqual(order, ["clone", "guard-eof"]);
});

test("original native bytes must match the guarded bytes in both length and every byte", async t => {
  for (const native of ["[]", "{", "{} "]) {
    const response = new Response("{}", { headers });
    const order = instrument(t, response, async () => encode(native).buffer); let calls = 0;
    t.mock.method(globalThis, "fetch", async () => { calls++; return response; });
    await assert.rejects(platformRequest(path, "GET"), unknown);
    assert.deepEqual(order, ["clone", "guard-eof", "native"]); assert.equal(calls, 1);
  }
});

test("native consumer errors remain one-shot fixed failures rather than falling back to guard bytes", async t => {
  const response = new Response("{}", { headers });
  const order = instrument(t, response, async () => { throw new Error(secret); }); let calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls++; return response; });
  await assert.rejects(platformRequest(path, "GET"), unknown);
  assert.deepEqual(order, ["clone", "guard-eof", "native"]); assert.equal(calls, 1);
});

test("parent abort exits a held guard even when synthetic fetch ignores its signal, with no native read", async t => {
  const parent = new AbortController(); let cancelled = 0, fetchSignal;
  const response = new Response(new ReadableStream({ cancel() { cancelled++; } }), { headers });
  const order = instrument(t, response);
  t.mock.method(globalThis, "fetch", async (_path, config) => { fetchSignal = config.signal; return response; });
  const waiting = platformRequest(path, "GET", { signal: parent.signal }); await tick();
  parent.abort(new Error(secret)); await assert.rejects(waiting, unknown); await tick();
  // reader.cancel() may resolve its held read with done:true; the post-read abort
  // fence must still prevent native consumption from treating that as natural EOF.
  assert.equal(order[0], "clone"); assert.equal(order.includes("native"), false); assert.equal(cancelled, 1);
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
  assert.equal(getEventListeners(fetchSignal, "abort").length, 0);
});

test("denial headers precede clone and abort fences a held native consumer and its late result", async t => {
  const parent = new AbortController(), native = Promise.withResolvers();
  const started = Promise.withResolvers(), response = new Response("{}", { status: 403, headers });
  const order = instrument(t, response, () => { started.resolve(); return native.promise; }); let denied = 0, calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls++; return response; });
  const waiting = platformRequest(path, "GET", { signal: parent.signal, onDenied(status) {
    assert.equal(status, 403); assert.deepEqual(order, []); denied++;
  } });
  await started.promise;
  parent.abort(); await assert.rejects(waiting, error => unknown(error) && error.deniedStatus === 403);
  native.resolve(encode("{}").buffer); await tick();
  assert.equal(denied, 1); assert.equal(calls, 1);
  assert.deepEqual(order, ["clone", "guard-eof", "native"]);
  assert.equal(getEventListeners(parent.signal, "abort").length, 0);
});

test("both tee cancellations actually settle, release the owned guard and cancel the source only once", async t => {
  for (const rejectedCancel of [false, true]) {
    const parent = new AbortController(), settled = new Set();
    let sourceCancels = 0, guarded, fetchSignal;
    const source = new ReadableStream({ cancel() {
      sourceCancels++;
      if (rejectedCancel) return Promise.reject(new Error(secret));
    } });
    const response = new Response(source, { headers }), clone = response.clone.bind(response);
    t.mock.method(response, "clone", () => {
      guarded = clone(); const getReader = guarded.body.getReader.bind(guarded.body);
      // clone replaces response.body with the other tee branch. Observe that actual
      // branch, not the previously captured upstream source stream.
      const originalCancel = response.body.cancel.bind(response.body);
      t.mock.method(response.body, "cancel", (...args) => originalCancel(...args).then(
        value => { settled.add("original"); return value; },
        error => { settled.add("original"); throw error; }));
      t.mock.method(guarded.body, "getReader", () => {
        const reader = getReader(), cancel = reader.cancel.bind(reader);
        t.mock.method(reader, "cancel", (...args) => cancel(...args).then(
          value => { settled.add("guard"); return value; },
          error => { settled.add("guard"); throw error; }));
        return reader;
      });
      return guarded;
    });
    t.mock.method(globalThis, "fetch", async (_path, config) => { fetchSignal = config.signal; return response; });
    const waiting = platformRequest(path, "GET", { signal: parent.signal }); await tick();
    parent.abort(); await assert.rejects(waiting, unknown); await tick();
    assert.equal(sourceCancels, 1);
    assert.deepEqual([...settled].sort(), ["guard", "original"]);
    assert.equal(guarded.body.locked, false, "the reader this transport actually owns must be released");
    assert.equal(getEventListeners(parent.signal, "abort").length, 0);
    assert.equal(getEventListeners(fetchSignal, "abort").length, 0);
  }
});
