import assert from "node:assert/strict";
import test from "node:test";
import { ATTACHMENT_READ_TIMEOUT_MS, RUNTIME_MUTATION_TIMEOUT_MS, requestRuntimeJson,
  parseAttachmentDetails, parseDetachResult, parseRunResult, RuntimeHttpError } from "../src/features/ebpf/runtimeRequest.ts";

const run = { success: false, stage: "compile", message: "compiler rejection", compile_stdout: "out", compile_stderr: "err", load_stdout: "", load_stderr: "", pin_path: null };
const attached = { pin_path: "/sys/fs/bpf/fixture/program", source: "int fixture;", program_name: "fixture" };
const detached = { ok: true, clean: true, message: "removed", detached: [attached.pin_path], safety_notes: [] };

test("runtime mutations use one private non-redirecting credentialed POST without adding authority", async t => {
  const calls = [], parent = new AbortController(), body = { code: "int fixture;", lab_id: "01-fixture" };
  t.mock.method(globalThis, "fetch", (url, init) => { calls.push({ url, ...init }); return Response.json(run); });
  assert.deepEqual(await requestRuntimeJson("https://engine.invalid/ebpf/run", parent.signal, body), run);
  assert.equal(calls.length, 1); assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].credentials, "include"); assert.equal(calls[0].cache, "no-store"); assert.equal(calls[0].redirect, "error");
  assert.deepEqual(JSON.parse(calls[0].body), body);
});

test("attachment reconciliation is a private GET, never an implicit detach", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (url, init) => { calls.push({ url, ...init }); return Response.json({ attachments: [] }); });
  await requestRuntimeJson("https://engine.invalid/ebpf/attachments/details", new AbortController().signal, undefined, ATTACHMENT_READ_TIMEOUT_MS);
  assert.equal(calls.length, 1); assert.equal(calls[0].method, "GET"); assert.equal(calls[0].body, undefined);
  assert.equal(calls[0].cache, "no-store"); assert.equal(calls[0].redirect, "error");
});

test("a pre-cancelled runtime request never calls fetch", async t => {
  const fetch = t.mock.method(globalThis, "fetch", () => { throw new Error("must not dispatch"); });
  const parent = new AbortController(); parent.abort();
  await assert.rejects(requestRuntimeJson("https://engine.invalid/ebpf/run", parent.signal, {}), { name: "AbortError" });
  assert.equal(fetch.mock.callCount(), 0);
});

test("runtime deadline covers a hung JSON body and releases its parent listener", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const parent = new AbortController(), calls = [];
  const removed = t.mock.method(parent.signal, "removeEventListener");
  t.mock.method(globalThis, "fetch", (_url, init) => {
    calls.push(init); return { ok: true, json: () => new Promise((_resolve, reject) => init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true })) };
  });
  const pending = requestRuntimeJson("https://engine.invalid/ebpf/run", parent.signal, {});
  await Promise.resolve(); t.mock.timers.tick(RUNTIME_MUTATION_TIMEOUT_MS - 1); assert.equal(calls[0].signal.aborted, false);
  t.mock.timers.tick(1); await assert.rejects(pending, { name: "TimeoutError" });
  assert.equal(removed.mock.callCount(), 1); assert.equal(parent.signal.aborted, false);
});

test("navigation aborts an active request but cannot publish an abort-ignoring late body", async t => {
  let release, request;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return new Promise(resolve => { release = resolve; }); });
  const parent = new AbortController(), pending = requestRuntimeJson("https://engine.invalid/ebpf/detach", parent.signal, { pin_path: attached.pin_path });
  parent.abort(); assert.equal(request.signal.aborted, true);
  release(Response.json(detached)); await assert.rejects(pending, { name: "AbortError" });
});

test("completed runtime reads release their deadline and cancellation linkage", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let request;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return Response.json({ attachments: [] }); });
  const parent = new AbortController();
  await requestRuntimeJson("https://engine.invalid/ebpf/attachments/details", parent.signal, undefined, ATTACHMENT_READ_TIMEOUT_MS);
  parent.abort(); t.mock.timers.tick(RUNTIME_MUTATION_TIMEOUT_MS);
  assert.equal(request.signal.aborted, false);
});

test("HTTP, network and malformed-body failures do not retry a runtime mutation", async t => {
  let count = 0;
  for (const status of [400, 401, 403, 408, 413, 429, 500, 503]) {
    t.mock.method(globalThis, "fetch", () => { count++; return Response.json({ message: "fixture rejection" }, { status }); });
    await assert.rejects(requestRuntimeJson("https://engine.invalid/ebpf/run", new AbortController().signal, {}), error => {
      assert.ok(error instanceof RuntimeHttpError); assert.equal(error.status, status);
      assert.deepEqual(error.payload, { message: "fixture rejection" }); return true;
    });
  }
  t.mock.method(globalThis, "fetch", () => { count++; throw new TypeError("offline"); });
  await assert.rejects(requestRuntimeJson("https://engine.invalid/ebpf/run", new AbortController().signal, {}), /offline/);
  t.mock.method(globalThis, "fetch", () => { count++; return new Response("{broken"); });
  await assert.rejects(requestRuntimeJson("https://engine.invalid/ebpf/run", new AbortController().signal, {}), SyntaxError);
  assert.equal(count, 10);
});

test("normal compiler rejection and valid debug records retain their result fields", () => {
  assert.deepEqual(parseRunResult(run), run);
  const debug = { mode: "kernel-trace", session_id: "fixture", requested_lines: [1], instrumented_lines: [], rejected: [{ line: 1, reason: "fixture" }] };
  assert.deepEqual(parseRunResult({ ...run, debug }), { ...run, debug });
});

test("malformed run and nested debug fields cannot reach result rendering", () => {
  for (const value of [null, [], {}, { ...run, success: "false" }, { ...run, message: {} }, { ...run, pin_path: {} },
    { ...run, debug: {} }, { ...run, debug: { mode: "kernel-trace", requested_lines: [], instrumented_lines: [], rejected: [null] } }]) {
    assert.throws(() => parseRunResult(value));
  }
});

test("detach parsing never fabricates clean or changes explicit warning fields", () => {
  assert.deepEqual(parseDetachResult(detached), detached);
  const { clean, ...legacy } = detached; assert.equal(parseDetachResult(legacy).clean, undefined);
  assert.equal(parseDetachResult({ ...detached, clean: false }).clean, false);
  for (const value of [null, {}, { ...detached, ok: "true" }, { ...detached, clean: "true" }, { ...detached, detached: null },
    { ...detached, detached: [attached.pin_path, attached.pin_path] }, { ...detached, safety_notes: [null] }]) assert.throws(() => parseDetachResult(value));
});

test("only a valid distinct inventory may be reported as ready, including a real empty result", () => {
  assert.deepEqual(parseAttachmentDetails({ attachments: [] }), []);
  assert.deepEqual(parseAttachmentDetails({ attachments: [attached] }), [attached]);
  for (const value of [null, {}, { attachments: [null] }, { attachments: [{}] }, { attachments: [{ ...attached, source: {} }] },
    { attachments: [{ ...attached, pin_path: " " }] }, { attachments: [attached, attached] }]) assert.throws(() => parseAttachmentDetails(value));
});
