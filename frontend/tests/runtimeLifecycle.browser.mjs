import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  bundle = await buildFixture(new URL("./fixtures/runtimeLifecycle.mjs", import.meta.url));
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const pin = "/sys/fs/bpf/fixture/program-a";
const attached = { pin_path: pin, source: "int fixture;", program_name: "fixture" };
const runResult = { success: true, stage: "run", message: "fixture loaded", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "", pin_path: pin };
const detached = { ok: true, clean: true, message: "detached", detached: [pin], safety_notes: [] };
async function run(callback) {
  const f = await setupFixture(browser, bundle);
  f.act = fn => f.page.evaluate(fn);
  f.render = (options = {}) => f.page.evaluate(value => fixture.render(value), options);
  f.outcome = id => f.page.evaluate(key => fixture.outcomes[key], id);
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); }
  finally { await f.close(); }
}

test("run admission closes the same-turn duplicate dispatch gap", () => run(async f => {
  await f.render(); await f.act(() => { fixture.invoke("runEbpf", "first"); fixture.invoke("runEbpf", "second"); });
  assert.equal((await f.requests()).filter(item => item.url.endsWith("/run")).length, 1);
}));

test("pre-cancelled confirmations never dispatch a kernel run", () => run(async f => {
  await f.render(); await f.act(() => { fixture.tokens.cancelled = new AbortController(); fixture.tokens.cancelled.abort(); fixture.invoke("runEbpf", "cancelled"); });
  assert.equal((await f.requests()).length, 0);
}));

test("unmount cancels a run and late completion starts no follow-up reads", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.render(false);
  const before = await f.act(() => fixture.calls.length);
  await f.respond(0, runResult);
  assert.equal((await f.requests())[0].aborted, true);
  assert.equal(await f.act(() => fixture.calls.length), before);
}));

test("late run results cannot populate another lab after navigation", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf"));
  await f.render({ lab: "02-other", navigation: "/ebpf?lab=02-other" }); await f.respond(0, runResult);
  assert.equal((await f.snapshot()).result, null); assert.equal((await f.requests())[0].aborted, true);
}));

test("editing a completed run's source hides its old result and explains retained kernel state", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.respond(0, runResult);
  assert.equal((await f.snapshot()).result.message, runResult.message);
  await f.act(() => fixture.changeCode("int changed_source;"));
  assert.equal((await f.snapshot()).result, null); assert.ok((await f.snapshot()).runtimeNotice);
}));

test("slow attachment refresh cannot keep a completed run stuck in running state", () => run(async f => {
  await f.render(); await f.act(() => { fixture.holdAttachments = true; fixture.invoke("runEbpf"); });
  await f.respond(0, runResult);
  assert.equal((await f.snapshot()).running, false); assert.ok((await f.outcome("runEbpf"))?.resolved);
}));

test("run transport failure is unconfirmed and reaches the confirmation failure path without retry", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.reject(0);
  assert.ok((await f.outcome("runEbpf"))?.rejected); assert.match((await f.snapshot()).error, /runtimeUnconfirmed/);
  assert.equal((await f.requests()).filter(item => item.url.endsWith("/run")).length, 1);
}));

test("a stalled runtime mutation is bounded including its response body", () => run(async f => {
  await f.render(); await f.act(() => { fixture.honorAbort = true; fixture.invoke("runEbpf"); });
  await f.advance(330000);
  assert.equal((await f.requests())[0].aborted, true); assert.equal((await f.snapshot()).running, false);
  assert.match((await f.snapshot()).error, /runtimeUnconfirmed/);
}));

test("UTF-8 oversized source never crosses the run boundary", () => run(async f => {
  await f.render(); await f.act(() => fixture.changeCode("你".repeat(90000)));
  await f.act(() => fixture.invoke("runEbpf"));
  assert.equal((await f.requests()).length, 0); assert.ok((await f.snapshot()).error);
}));

test("an old startup inventory cannot resurrect a pin after post-run refresh", () => run(async f => {
  await f.act(() => { fixture.holdAttachments = true; }); await f.render();
  assert.equal((await f.requests()).length, 2, "StrictMode starts two independent reads");
  await f.respond(1, { attachments: [attached] }); await f.act(() => fixture.invoke("runEbpf"));
  await f.respond(2, runResult); await f.respond(3, { attachments: [] });
  await f.respond(0, { attachments: [attached] });
  assert.deepEqual((await f.snapshot()).attachments, []);
}));

test("failed inventory refresh keeps the last list but cannot claim ready or empty success", () => run(async f => {
  await f.act(() => { fixture.attachments = [{ pin_path: "/sys/fs/bpf/fixture/program-a", source: "int fixture;", program_name: "fixture" }]; });
  await f.render(); await f.act(() => { fixture.holdAttachments = true; fixture.invoke("runEbpf"); });
  await f.respond(0, runResult); await f.respond(1, { message: "offline" }, 503);
  assert.deepEqual((await f.snapshot()).attachments, [pin]); assert.equal((await f.snapshot()).attachmentState, "error");
}));

test("malformed attachment rows cannot crash the controller", () => run(async f => {
  await f.act(() => { fixture.attachments = [null]; }); await f.render(); await f.advance(1);
  assert.deepEqual(f.errors, []); assert.equal((await f.snapshot()).attachmentState, "error");
}));

test("attachment reads have a deadline and cancel on unmount", () => run(async f => {
  await f.act(() => { fixture.honorAbort = true; fixture.holdAttachments = true; }); await f.render();
  await f.advance(20000);
  assert.equal((await f.requests())[1].aborted, true); assert.equal((await f.snapshot()).attachmentState, "error");
}));

test("detach admission prevents duplicate mutations and simultaneous runs", () => run(async f => {
  await f.render(); await f.act(() => { fixture.invoke("detach", "first", "/sys/fs/bpf/fixture/program-a"); fixture.invoke("detach", "second", "/sys/fs/bpf/fixture/program-a"); fixture.invoke("runEbpf"); });
  assert.equal((await f.requests()).length, 1); assert.equal((await f.snapshot()).detaching, true);
}));

test("missing clean confirmation or unclean empty notes must not be successful cleanup", () => run(async f => {
  await f.render();
  for (const [index, clean] of [undefined, false].entries()) {
    await f.page.evaluate(i => fixture.invoke("detach", "detach-" + i, "/sys/fs/bpf/fixture/program-a"), index);
    await f.respond(index, { ...detached, clean });
    assert.ok((await f.outcome("detach-" + index))?.rejected); assert.ok((await f.snapshot()).error);
  }
}));

test("verified detach retires the old result pin instead of leaving its quick action live", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.respond(0, runResult);
  await f.act(() => fixture.invoke("detach", "detach", "/sys/fs/bpf/fixture/program-a")); await f.respond(1, detached);
  assert.equal((await f.snapshot()).result.pin_path, null);
}));

test("late detach after navigation cannot rewrite a new run or clear its busy state", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("detach", "old", "/sys/fs/bpf/fixture/program-a"));
  await f.render({ lab: "02-other", navigation: "/ebpf?lab=02-other" }); await f.act(() => fixture.invoke("runEbpf", "new"));
  await f.respond(0, detached); assert.equal((await f.snapshot()).running, true);
  await f.respond(1, { ...runResult, message: "new run", pin_path: "/sys/fs/bpf/fixture/program-b" });
  assert.equal((await f.requests())[0].aborted, true); assert.equal((await f.snapshot()).result.message, "new run");
}));

test("malformed successful run bodies cannot be published as runtime results", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.respond(0, { success: true, debug: {} });
  assert.equal((await f.snapshot()).result, null); assert.ok((await f.outcome("runEbpf"))?.rejected);
}));

test("structured validation rejection remains a normal run report", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf"));
  await f.respond(0, { ...runResult, success: false, stage: "validation", message: "fixture invalid source", pin_path: null }, 400);
  assert.equal((await f.snapshot()).result?.stage, "validation");
  assert.equal((await f.snapshot()).error, "fixture invalid source"); assert.ok((await f.outcome("runEbpf"))?.resolved);
}));

test("explicit authentication rejection is not mislabeled as an uncertain kernel mutation", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf")); await f.respond(0, { message: "sign in required" }, 401);
  assert.equal((await f.snapshot()).error, "sign in required"); assert.ok((await f.outcome("runEbpf"))?.rejected);
}));

test("ordinary compiler rejection keeps the report and no automatic retry or detach", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf"));
  await f.respond(0, { ...runResult, success: false, stage: "compile", message: "compiler rejected source", pin_path: null });
  assert.equal((await f.snapshot()).result.stage, "compile"); assert.equal((await f.snapshot()).error, "compiler rejected source");
  assert.ok((await f.outcome("runEbpf"))?.resolved); assert.equal((await f.requests()).length, 1);
}));

test("source edits during a dispatched run discard its output but still reconcile owner inventory", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf"));
  await f.act(() => fixture.changeCode("int changed_source;"));
  assert.equal((await f.requests())[0].aborted, false, "editing is not an automatic cancellation or detach");
  const before = await f.act(() => fixture.calls.filter(item => item.url.endsWith("/attachments/details")).length);
  await f.respond(0, runResult);
  assert.equal((await f.snapshot()).result, null); assert.ok((await f.snapshot()).runtimeNotice);
  assert.equal(await f.act(() => fixture.calls.filter(item => item.url.endsWith("/attachments/details")).length), before + 1);
  assert.equal((await f.requests()).length, 1);
}));

test("a cancelled old run cannot clear the new run's busy flag or replace its error", () => run(async f => {
  await f.render(); await f.act(() => fixture.invoke("runEbpf", "old"));
  await f.render({ lab: "02-other", navigation: "/ebpf?lab=02-other" }); await f.act(() => fixture.invoke("runEbpf", "new"));
  await f.reject(0); assert.equal((await f.snapshot()).running, true); assert.equal((await f.snapshot()).error, null);
  await f.respond(1, runResult); assert.equal((await f.snapshot()).result.message, runResult.message);
}));

test("inventory refresh remains latest-wins and aborts when leaving the editor", () => run(async f => {
  await f.render(); await f.act(() => { fixture.holdAttachments = true; void fixture.controller.refreshAttachments(); void fixture.controller.refreshAttachments(); });
  await f.respond(1, { attachments: [] }); await f.respond(0, { attachments: [attached] });
  assert.deepEqual((await f.snapshot()).attachments, []); assert.equal((await f.requests())[0].aborted, true);
  await f.act(() => { void fixture.controller.refreshAttachments(); }); await f.render(false);
  assert.equal((await f.requests())[2].aborted, true);
}));
