import assert from "node:assert/strict";
import test from "node:test";
import { runCompilerCheck } from "../src/features/ebpf/compilerCheck.ts";

const engine = "https://teacher.invalid";
const result = { ok: true, message: "checked", diagnostics: [], stdout: "", stderr: "" };
const job = (state, report = null) => ({ job_id: "fixture-job", state, message: state, result: report });
const flush = async () => { for (let index = 0; index < 30; index++) await Promise.resolve(); };
const waiting = signal => new Promise((_resolve, reject) => {
  if (signal.aborted) reject(signal.reason);
  else signal.addEventListener("abort", () => reject(signal.reason), { once: true });
});

function fixture(t, handler) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  t.mock.method(globalThis, "fetch", (url, init) => {
    calls.push({ url, ...init });
    return handler(url, init, calls.length);
  });
  return calls;
}

test("local check uses private, non-redirecting credentialed transport without running code", async t => {
  const calls = fixture(t, () => Response.json(result));
  assert.deepEqual(await runCompilerCheck("int source;", engine, "local", new AbortController().signal), result);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${engine}/ebpf/check`);
  assert.deepEqual(JSON.parse(calls[0].body), { code: "int source;" });
  for (const call of calls) {
    assert.equal(call.cache, "no-store"); assert.equal(call.redirect, "error");
    assert.equal(call.credentials, "include"); assert.equal(call.method, "POST");
  }
  t.mock.timers.tick(35000);
  assert.equal(calls[0].signal.aborted, false, "completed work removes its deadline timer");
});

test("authentication, capacity and network errors never retry or fall back from an Agent", async t => {
  let status = 401;
  const calls = fixture(t, () => {
    if (status === 0) throw new TypeError("offline");
    return Response.json({ message: `rejected ${status}` }, { status });
  });
  for (status of [401, 403, 429, 503, 0]) {
    await assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal));
  }
  assert.equal(calls.length, 5);
  assert.ok(calls.every(call => call.url === `${engine}/ebpf/check/remote`));
});

test("pre-cancelled work dispatches no request", async t => {
  const calls = fixture(t, () => assert.fail("must not dispatch"));
  const parent = new AbortController(); parent.abort();
  await assert.rejects(runCompilerCheck("int source;", engine, "local", parent.signal), { name: "AbortError" });
  assert.equal(calls.length, 0);
});

test("local deadline also aborts a stalled JSON body after successful headers", async t => {
  const calls = fixture(t, (_url, { signal }) => ({ ok: true, json: () => waiting(signal) }));
  const pending = runCompilerCheck("int source;", engine, "local", new AbortController().signal);
  const rejected = assert.rejects(pending, { name: "TimeoutError" });
  await flush(); t.mock.timers.tick(19999); assert.equal(calls[0].signal.aborted, false);
  t.mock.timers.tick(1); await rejected;
  assert.equal(calls[0].signal.aborted, true);
});

test("remote submission has a deadline even before a job ID is known", async t => {
  const calls = fixture(t, (_url, { signal }) => waiting(signal));
  const rejected = assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal), { name: "TimeoutError" });
  t.mock.timers.tick(35000); await rejected;
  assert.equal(calls.length, 1);
});

test("remote submission and stalled polling share one deadline and cancel only the known job", async t => {
  let submit;
  const calls = fixture(t, (url, { signal }, index) => {
    if (index === 1) return new Promise(resolve => { submit = resolve; });
    if (url.endsWith("/cancel")) return Response.json({ ok: true });
    return waiting(signal);
  });
  const rejected = assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal), { name: "TimeoutError" });
  t.mock.timers.tick(10000); submit(Response.json(job("queued"))); await flush();
  t.mock.timers.tick(25000); await rejected;
  assert.equal(calls.length, 3);
  assert.equal(calls[1].signal.aborted, true);
  assert.equal(calls[2].signal.aborted, false, "best-effort cancellation has its own deadline");
  assert.equal(calls[2].keepalive, true);
  assert.deepEqual(JSON.parse(calls[2].body), { job_id: "fixture-job" });
});

test("nonterminal remote responses wait 500 ms and do not accept a premature result", async t => {
  const calls = fixture(t, (_url, _init, index) => Response.json(index === 1
    ? job("queued") : index === 2 ? job("claimed", { ...result, message: "premature" }) : job("succeeded", result)));
  const pending = runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal);
  await flush(); assert.equal(calls.length, 2);
  t.mock.timers.tick(499); await flush(); assert.equal(calls.length, 2);
  t.mock.timers.tick(1); assert.deepEqual(await pending, result);
  assert.equal(calls.length, 3);
  assert.ok(calls.every(call => call.url.startsWith(`${engine}/ebpf/check/remote`)));
});

test("cancellation between polls stops the delay and sends exactly one private cancellation", async t => {
  const calls = fixture(t, () => Response.json(job("queued")));
  const parent = new AbortController();
  const rejected = assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", parent.signal), { name: "AbortError" });
  await flush(); parent.abort(); await rejected;
  t.mock.timers.tick(35000); await flush();
  assert.equal(calls.length, 3);
  assert.equal(calls[2].url, `${engine}/ebpf/check/remote/cancel`);
  assert.equal(calls[2].cache, "no-store"); assert.equal(calls[2].redirect, "error");
  assert.equal(calls[2].credentials, "include");
});

test("cancelled/expired jobs are not compiler reports and known terminal jobs are not cancelled again", async t => {
  let state;
  const calls = fixture(t, (_url, init) => Response.json(init.method === "POST" ? job("queued") : job(state, { ...result, ok: false })));
  for (state of ["cancelled", "expired"]) {
    await assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal), new RegExp(state));
  }
  assert.equal(calls.length, 4);
  assert.ok(calls.every(call => !call.url.endsWith("/cancel")));
});

test("compiler rejection remains a normal report, not an infrastructure failure", async t => {
  const failed = { ...result, ok: false, message: "syntax error" };
  const calls = fixture(t, (_url, _init, index) => Response.json(index === 1 ? job("queued") : job("failed", failed)));
  assert.deepEqual(await runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal), failed);
  assert.equal(calls.length, 2);
});

test("malformed JSON and missing remote job IDs fail without caching or a fabricated cancellation", async t => {
  let malformed = true;
  const calls = fixture(t, () => malformed ? new Response("not JSON") : Response.json({ state: "queued" }));
  await assert.rejects(runCompilerCheck("int source;", engine, "local", new AbortController().signal));
  malformed = false;
  await assert.rejects(runCompilerCheck("int source;", engine, "agent:chosen", new AbortController().signal), /job ID/);
  assert.equal(calls.length, 2);
});
