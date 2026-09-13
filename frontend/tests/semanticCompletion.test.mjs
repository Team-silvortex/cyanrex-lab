import assert from "node:assert/strict";
import test from "node:test";
import { createSemanticCompletion } from "../src/utils/semanticCompletion.ts";

const items = [{ label: "field", insert_text: "field", detail: "fixture", kind: "field" }];
function event() {
  const listeners = new Set();
  return { on: callback => { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; },
    fire: () => { for (const callback of listeners) callback(); }, get size() { return listeners.size; } };
}
function fixture(t) {
  const content = event(), removed = event(), changed = event(), disposed = event(), cancelled = event();
  const model = { source: "int field;", version: 1, dead: false, uri: { toString: () => "inmemory:///fixture" },
    isDisposed() { return this.dead; }, getValue() { return this.source; }, getVersionId() { return this.version; },
    onDidChangeContent: content.on, onWillDispose: removed.on,
    change(source) { this.source = source; this.version++; content.fire(); } };
  const editor = { model, getModel() { return this.model; }, onDidChangeModel: changed.on, onDidDispose: disposed.on };
  const token = { isCancellationRequested: false, onCancellationRequested: cancelled.on,
    cancel() { this.isCancellationRequested = true; cancelled.fire(); } };
  const client = createSemanticCompletion(editor, "https://engine.invalid", "selected-headers");
  t.after(() => client.dispose());
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  const calls = [];
  let respond = () => Response.json({ ok: true, items, message: "fixture" });
  t.mock.method(globalThis, "fetch", (url, init) => { calls.push({ url, ...init }); return respond(init); });
  return { client, model, editor, token, calls, content, removed, changed, disposed, cancelled,
    respond: fn => { respond = fn; }, request: (column = 3) => client.request(model, { lineNumber: 1, column }, token) };
}

test("semantic request stays local and sends only source/cursor with private transport", async t => {
  const f = fixture(t); assert.deepEqual(await f.request(), items);
  const call = f.calls[0]; assert.equal(call.url, "https://engine.invalid/ebpf/complete");
  assert.deepEqual(JSON.parse(call.body), { code: "int field;", line: 1, column: 3 });
  assert.equal(call.credentials, "include"); assert.equal(call.cache, "no-store"); assert.equal(call.redirect, "error");
  assert.equal(f.content.size, 0); assert.equal(f.removed.size, 0); assert.equal(f.cancelled.size, 0);
  t.mock.timers.tick(10000); assert.equal(call.signal.aborted, false, "finished requests release their deadline");
});

test("completion cache expires exactly at five seconds and respects cursor/source identity", async t => {
  const f = fixture(t); await f.request(); t.mock.timers.tick(4999); await f.request(); assert.equal(f.calls.length, 1);
  t.mock.timers.tick(1); await f.request(); assert.equal(f.calls.length, 2);
  await f.request(4); assert.equal(f.calls.length, 3);
  f.model.change("int other;"); await f.request(4); assert.equal(f.calls.length, 4);
});

test("completion cache remains bounded to eighteen entries", async t => {
  const f = fixture(t);
  for (let i = 1; i <= 19; i++) await f.request(i);
  await f.request(2); assert.equal(f.calls.length, 19);
  await f.request(1); assert.equal(f.calls.length, 20);
});

test("foreign, disposed and pre-cancelled models dispatch nothing", async t => {
  const f = fixture(t);
  assert.equal(await f.client.request({ ...f.model }, { lineNumber: 1, column: 3 }, f.token), null);
  f.model.dead = true; assert.equal(await f.request(), null); f.model.dead = false;
  f.token.cancel(); assert.equal(await f.request(), null); assert.equal(f.calls.length, 0);
});

test("empty and UTF-8 oversized source remains snippet-only without dispatch", async t => {
  const f = fixture(t);
  for (const source of [" ", "你".repeat(90000), "x".repeat(262145)]) {
    f.model.change(source); assert.deepEqual(await f.request(), []);
  }
  assert.equal(f.calls.length, 0);
});

test("superseded cursor requests discard even already-delivered responses", async t => {
  const f = fixture(t), replies = [];
  f.respond(() => new Promise(resolve => replies.push(resolve)));
  const first = f.request(3), second = f.request(4);
  assert.equal(f.calls[0].signal.aborted, true); assert.equal(f.calls[1].signal.aborted, false);
  replies[1](Response.json({ ok: true, items, message: "new" })); assert.deepEqual(await second, items);
  replies[0](Response.json({ ok: true, items, message: "old" })); assert.equal(await first, null);
});

test("owner disposal aborts active work and releases listeners without publishing a late result", async t => {
  const f = fixture(t); let release;
  f.respond(() => new Promise(resolve => { release = resolve; }));
  const pending = f.request(); f.disposed.fire(); assert.equal(f.calls[0].signal.aborted, true);
  release(Response.json({ ok: true, items })); assert.equal(await pending, null);
  assert.equal(f.changed.size, 0); assert.equal(f.disposed.size, 0); assert.equal(f.content.size, 0);
});

test("model replacement invalidates pending work and all model-local cache reuse", async t => {
  const f = fixture(t); await f.request(); f.changed.fire(); await f.request(); assert.equal(f.calls.length, 2);
  let release; f.respond(() => new Promise(resolve => { release = resolve; }));
  const pending = f.request(4); f.editor.model = null; f.changed.fire();
  release(Response.json({ ok: true, items })); assert.equal(await pending, null);
});

test("a stalled semantic JSON body has a ten-second whole-request deadline", async t => {
  const f = fixture(t);
  f.respond(({ signal }) => ({ ok: true, json: () => new Promise((_resolve, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true })) }));
  const pending = f.request(); await Promise.resolve(); await Promise.resolve();
  t.mock.timers.tick(10000); assert.deepEqual(await pending, []); assert.equal(f.calls[0].signal.aborted, true);
});

test("network/HTTP failures never retry or poison semantic cache", async t => {
  const f = fixture(t);
  f.respond(() => { throw new TypeError("offline"); }); assert.deepEqual(await f.request(), []);
  for (const status of [401, 403, 429, 503]) {
    f.respond(() => Response.json({ message: "unavailable" }, { status })); assert.deepEqual(await f.request(), []);
  }
  f.respond(() => Response.json({ ok: true, items })); assert.deepEqual(await f.request(), items);
  assert.equal(f.calls.length, 6);
});

test("malformed semantic payloads remain snippet-only instead of escaping into providers", async t => {
  const f = fixture(t);
  for (const body of [null, {}, { ok: true, items: null }, { ok: true, items: [null] }, { ok: true, items: [{ label: 1 }] }]) {
    f.respond(() => Response.json(body)); assert.deepEqual(await f.request(), []);
  }
});
