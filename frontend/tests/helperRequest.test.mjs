import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import test from "node:test";

const moduleUrl = new URL("../src/features/helper/helperRequest.ts", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, next) {
  return context.parentURL === moduleUrl && specifier === "../../transport/privateRequest"
    ? next(new URL("../src/transport/privateRequest.ts", import.meta.url).href, context) : next(specifier, context);
} });
let helper;
try { helper = await import(moduleUrl); } finally { resolver.deregister(); }
const { parseEnvironmentReport, parseRunnerStatus, readHelperSnapshot, MAX_HELPER_RESPONSE_BYTES, HELPER_CHECK_TIMEOUT_MS } = helper;
const environment = () => ({ overall_ok: true, generated_at: "2026-10-18T00:00:00Z", runtime_mode: "native-linux",
  runtime_guidance: "Native Linux backend; eBPF programs target the host kernel.",
  checks: [{ name: "clang", ok: true, detail: "clang fixture\nRaw process output stays exact." }] });
const runner = () => ({ mode: "local_process", isolation: "shared_kernel", instance_id: "fixture-instance",
  max_concurrent: 2, max_per_user: 1, active_total: 0, active_for_current_user: 0, available_slots: 2, execution_timeout_seconds: 45 });
const url = "https://engine.invalid";
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };

test("helper decoders preserve bounded raw reports without granting unknown response fields authority", () => {
  const report = environment(), status = runner();
  assert.deepEqual(parseEnvironmentReport({ ...report, extra: "ignored" }), report);
  assert.deepEqual(parseRunnerStatus({ ...status, extra: "ignored" }), status);
  for (const runtime_mode of ["native-linux", "wsl2", "docker"]) assert.equal(parseEnvironmentReport({ ...report, runtime_mode }).runtime_mode, runtime_mode);
  const failed = { ...report, overall_ok: false, checks: [{ ...report.checks[0], ok: false }] };
  assert.deepEqual(parseEnvironmentReport(failed), failed);
  assert.equal(parseRunnerStatus({ ...status, mode: "future_mode", isolation: "future_isolation" }).mode, "future_mode");
});

test("malformed or contradictory environment reports fail before entering rendering", () => {
  const report = environment();
  for (const invalid of [null, [], {}, { ...report, overall_ok: "true" }, { ...report, overall_ok: false },
    { ...report, runtime_mode: "unknown" }, { ...report, generated_at: "yesterday" },
    { ...report, generated_at: "2026-02-30T00:00:00Z" }, { ...report, generated_at: "2026-10-18T24:00:00Z" },
    { ...report, checks: null }, { ...report, checks: [] }, { ...report, checks: [null] },
    { ...report, checks: [report.checks[0], report.checks[0]] }, { ...report, checks: Array.from({ length: 33 }, (_, i) => ({ name: String(i), ok: true, detail: "" })) },
    { ...report, checks: [{ name: "", ok: true, detail: "" }] }, { ...report, checks: [{ name: "clang", ok: "true", detail: "" }] },
    { ...report, checks: [{ name: "clang", ok: true, detail: "a".repeat(8193) }] }, { ...report, runtime_guidance: "a".repeat(2049) }]) {
    assert.throws(() => parseEnvironmentReport(invalid), /Invalid helper response/);
  }
});

test("Runner counters are finite bounded integers with internally consistent capacities", () => {
  const status = runner();
  for (const invalid of [null, [], {}, { ...status, max_concurrent: 0 }, { ...status, max_concurrent: NaN },
    { ...status, max_per_user: 3 }, { ...status, active_total: 3 }, { ...status, active_for_current_user: 1 },
    { ...status, available_slots: 1 }, { ...status, active_total: -1 }, { ...status, active_total: 0.5 },
    { ...status, execution_timeout_seconds: 0 }, { ...status, execution_timeout_seconds: Infinity },
    { ...status, instance_id: "" }, { ...status, instance_id: "a".repeat(257) }, { ...status, mode: [] }]) {
    assert.throws(() => parseRunnerStatus(invalid), /Invalid helper response/);
  }
  assert.deepEqual(parseRunnerStatus({ ...status, active_total: 1, active_for_current_user: 1, available_slots: 1 }),
    { ...status, active_total: 1, active_for_current_user: 1, available_slots: 1 });
});

test("one explicit helper check performs exactly two private body-free GETs and publishes their complete pair", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (target, init) => { calls.push({ target, ...init });
    return Response.json(target.endsWith("/helper/environment") ? environment() : runner()); });
  assert.deepEqual(await readHelperSnapshot(url, new AbortController().signal), { report: environment(), runner: runner() });
  assert.deepEqual(calls.map(call => call.target).sort(), [`${url}/helper/environment`, `${url}/runner/status`].sort());
  for (const call of calls) {
    assert.equal(call.method, "GET"); assert.equal(call.credentials, "include"); assert.equal(call.cache, "no-store");
    assert.equal(call.redirect, "error"); assert.equal(call.body, undefined); assert.equal(call.headers, undefined);
  }
});

test("a completed half cannot expose a partial snapshot before the other report is decoded", async t => {
  const held = deferred(); let outcome;
  t.mock.method(globalThis, "fetch", target => target.endsWith("/helper/environment") ? Response.json(environment()) : held.promise);
  const checked = readHelperSnapshot(url, new AbortController().signal).then(value => { outcome = value; });
  await flush(); assert.equal(outcome, undefined);
  held.resolve(Response.json(runner())); await checked;
  assert.deepEqual(outcome, { report: environment(), runner: runner() });
});

test("failure of either half aborts its pending sibling and cannot become a partial success", async t => {
  for (const failure of ["/helper/environment", "/runner/status"]) {
    const requests = [], held = deferred();
    t.mock.method(globalThis, "fetch", (target, init) => { requests.push({ target, ...init });
      return target.endsWith(failure) ? new Response("private server detail", { status: 503 }) : held.promise; });
    await assert.rejects(readHelperSnapshot(url, new AbortController().signal), /Helper request failed/);
    assert.equal(requests.length, 2);
    assert.ok(requests.find(request => !request.target.endsWith(failure)).signal.aborted,
      "the pending sibling is cancelled; completed requests no longer own parent listeners");
    held.resolve(Response.json(failure === "/runner/status" ? environment() : runner())); await flush();
  }
});

test("status media type malformed JSON and invalid UTF-8 fail without rendering server error details", async t => {
  for (const response of [new Response("secret", { status: 403 }), new Response("{}", { headers: { "Content-Type": "text/html" } }),
    new Response("{bad", { headers: { "Content-Type": "application/json" } }),
    new Response(new Uint8Array([123, 34, 255, 34, 125]), { headers: { "Content-Type": "application/json" } })]) {
    const fetch = t.mock.method(globalThis, "fetch", target => target.endsWith("/helper/environment") ? response : Response.json(runner()));
    await assert.rejects(readHelperSnapshot(url, new AbortController().signal)); assert.equal(fetch.mock.callCount(), 2);
  }
});

test("advertised oversize bodies are rejected without consuming their stream", async t => {
  let pulls = 0, cancelled = 0;
  const body = new ReadableStream({ pull() { pulls++; }, cancel() { cancelled++; } }, { highWaterMark: 0 });
  t.mock.method(globalThis, "fetch", target => target.endsWith("/helper/environment")
    ? new Response(body, { headers: { "Content-Type": "application/json", "Content-Length": String(MAX_HELPER_RESPONSE_BYTES + 1) } }) : Response.json(runner()));
  await assert.rejects(readHelperSnapshot(url, new AbortController().signal));
  assert.equal(pulls, 0); assert.equal(cancelled, 1);
});

test("streamed bytes have an exact cap even without a trustworthy content length", async t => {
  const json = JSON.stringify(environment()), encoder = new TextEncoder();
  for (const extra of [0, 1]) {
    const text = json + " ".repeat(MAX_HELPER_RESPONSE_BYTES - encoder.encode(json).length + extra);
    let cancelled = 0;
    t.mock.method(globalThis, "fetch", target => target.endsWith("/helper/environment")
      ? new Response(new ReadableStream({ start(controller) { const bytes = encoder.encode(text);
        controller.enqueue(bytes.slice(0, 50)); controller.enqueue(bytes.slice(50)); controller.close(); }, cancel() { cancelled++; } }),
      { headers: { "Content-Type": "application/json" } }) : Response.json(runner()));
    const check = readHelperSnapshot(url, new AbortController().signal);
    if (extra) await assert.rejects(check); else assert.deepEqual(await check, { report: environment(), runner: runner() });
    assert.ok(cancelled <= 1);
  }
});

test("UTF-8 characters split over stream frames remain exact", async t => {
  const report = environment(); report.checks[0].detail = "原始过程输出☕\r\n";
  const bytes = new TextEncoder().encode(JSON.stringify(report));
  t.mock.method(globalThis, "fetch", target => target.endsWith("/helper/environment")
    ? new Response(new ReadableStream({ start(controller) { for (const byte of bytes) controller.enqueue(new Uint8Array([byte])); controller.close(); } }),
      { headers: { "Content-Type": "Application/JSON; charset=utf-8" } }) : Response.json(runner()));
  assert.deepEqual((await readHelperSnapshot(url, new AbortController().signal)).report, report);
});

test("the complete paired check is bounded through stalled headers and body with no retry", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const phase of ["headers", "body"]) {
    const held = deferred(), requests = []; let bodyCancelled = 0, outcome;
    const fetch = t.mock.method(globalThis, "fetch", (target, init) => { requests.push(init);
      if (target.endsWith("/runner/status")) return Response.json(runner());
      return phase === "headers" ? held.promise : new Response(new ReadableStream({ cancel() { bodyCancelled++; } }),
        { headers: { "Content-Type": "application/json" } }); });
    const checked = readHelperSnapshot(url, new AbortController().signal).then(value => { outcome = { value }; }, error => { outcome = { error }; });
    await flush(); t.mock.timers.tick(HELPER_CHECK_TIMEOUT_MS - 1); await flush(); assert.equal(outcome, undefined);
    t.mock.timers.tick(1); await flush(); const bounded = outcome;
    held.resolve(Response.json(environment())); await checked; await flush();
    assert.equal(bounded?.error?.name, "TimeoutError"); assert.equal(outcome, bounded);
    assert.equal(requests[0].signal.aborted, true); assert.equal(fetch.mock.callCount(), 2);
    if (phase === "body") assert.equal(bodyCancelled, 1);
  }
});

test("pre-cancel dispatches nothing and cancellation drops late abort-ignoring header results", async t => {
  const parent = new AbortController(), held = deferred(), reason = new Error("owner left");
  const fetch = t.mock.method(globalThis, "fetch", () => held.promise);
  let outcome;
  const checked = readHelperSnapshot(url, parent.signal).then(value => { outcome = { value }; }, error => { outcome = { error }; });
  parent.abort(reason); await flush(); const cancelled = outcome;
  held.resolve(Response.json(environment())); await checked; await flush();
  assert.equal(cancelled?.error, reason); assert.equal(outcome, cancelled); assert.equal(fetch.mock.callCount(), 2);
  await assert.rejects(readHelperSnapshot(url, parent.signal), error => error === reason);
  assert.equal(fetch.mock.callCount(), 2);
});
