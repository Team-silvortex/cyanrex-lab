import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  bundle = await buildFixture();
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

const report = (message = "fixture", ok = true) => ({
  ok, message, stdout: "", stderr: "", diagnostics: [{ line: 1, column: 1, end_column: 2, severity: "note", message }],
});
const remote = (state, result = null) => ({ job_id: "fixture-job", state, message: `fixture ${state}`, result });

async function run(t, callback) {
  const f = await setupFixture(browser, bundle);
  try { await callback(f); assert.deepEqual(f.errors, []); }
  finally { await f.close(); }
}

test("cached diagnostics never cross Engine URLs", t => run(t, async f => {
  await f.render(); await f.advance(); await f.respond(0, report("engine A"));
  assert.equal((await f.snapshot()).status, "passed");
  await f.render([{ engineUrl: "https://engine-b.invalid" }]); await f.advance();
  assert.equal((await f.requests()).length, 2);
  assert.equal((await f.requests())[1].url, "https://engine-b.invalid/ebpf/check");
  await f.respond(1, report("engine B", false));
  assert.equal((await f.snapshot()).diagnostics[0].message, "clang: engine B");
}));

test("remounting an editor does not reuse the previous editor or account's cache", t => run(t, async f => {
  await f.render(); await f.advance(); await f.respond(0, report("previous owner"));
  await f.render([]); await f.render(); await f.advance();
  assert.equal((await f.requests()).length, 2);
  assert.equal((await f.snapshot()).status, "checking");
}));

test("source and header context use unambiguous cache identities", t => run(t, async f => {
  await f.render([{ code: "int a;//b", headerContextKey: "c" }]);
  await f.advance(); await f.respond(0, report("old context"));
  await f.render([{ code: "int a;", headerContextKey: "b//c" }]); await f.advance();
  assert.equal((await f.requests()).length, 2);
}));

test("equal-length sources with the same old 32-bit hash do not share diagnostics", t => run(t, async f => {
  // Both previously hashed to 51fedefd-49 with local target and empty header context.
  await f.render([{ code: "int main(void) { return 0; } // a35ea510" }]);
  await f.advance(); await f.respond(0, report("first source"));
  await f.render([{ code: "int main(void) { return 0; } // bd17356c" }]); await f.advance();
  assert.equal((await f.requests()).length, 2);
}));

test("late success cannot replace current diagnostics or populate a stale cache", t => run(t, async f => {
  await f.render([{ code: "int old;" }]); await f.advance();
  await f.render([{ code: "int current;" }]); await f.advance();
  await f.respond(1, report("current", false));
  await f.respond(0, report("late old"));
  assert.equal((await f.snapshot()).diagnostics[0].message, "clang: current");
  assert.equal((await f.requests())[0].aborted, true);
  await f.render([{ code: "int old;" }]); await f.advance();
  assert.equal((await f.requests()).length, 3, "discarded work is not cached");
}));

test("late errors cannot erase the current diagnostics", t => run(t, async f => {
  await f.render([{ code: "int old;" }]); await f.advance();
  await f.render([{ code: "int current;" }]); await f.advance();
  await f.respond(1, report("current")); await f.reject(0);
  assert.equal((await f.snapshot()).status, "passed");
  assert.equal((await f.snapshot()).diagnostics[0].message, "clang: current");
}));

test("changing source removes old markers and status during debounce", t => run(t, async f => {
  await f.render(); await f.advance(); await f.respond(0, report("old error", false));
  await f.render([{ code: "int changed;" }]);
  assert.deepEqual(await f.snapshot(), { diagnostics: [], status: "idle" });
  await f.advance(699); assert.equal((await f.requests()).length, 1);
  await f.advance(1); assert.equal((await f.snapshot()).status, "checking");
}));

test("one editor's cancellation does not own another editor's request", t => run(t, async f => {
  await f.page.evaluate(() => { window.fixture.honorAbort = true; });
  await f.render([{ id: "first" }, { id: "second" }]); await f.advance();
  const survivor = (await f.requests()).length - 1;
  await f.render([{ id: "second" }]);
  assert.equal((await f.requests())[survivor].aborted, false, "remaining consumer must keep a live request");
  await f.respond(survivor, report("survivor"));
  assert.equal((await f.snapshot("second")).status, "passed");
}));

test("cancelled and expired remote jobs are unavailable even when they carry a result", t => run(t, async f => {
  for (const state of ["cancelled", "expired"]) {
    await f.render([{ code: `int ${state};`, target: "agent:compiler" }]); await f.advance();
    const index = (await f.requests()).length - 1;
    await f.respond(index, remote("queued"));
    await f.respond(index + 1, remote(state, report(state, false)));
    assert.equal((await f.snapshot()).status, "unavailable");
    assert.deepEqual((await f.snapshot()).diagnostics, []);
    assert.equal((await f.requests()).some(request => request.url.endsWith("/cancel")), false, "terminal jobs need no cancellation");
  }
}));

test("remote submission and a stalled poll each have a bounded 35-second total wait", t => run(t, async f => {
  await f.page.evaluate(() => { window.fixture.honorAbort = true; });
  await f.render([{ target: "agent:compiler" }]); await f.advance();
  await f.advance(35001);
  assert.equal((await f.snapshot()).status, "unavailable", "stalled submission must time out");
  assert.equal((await f.requests())[0].aborted, true);
  assert.equal((await f.requests()).length, 1, "unknown job IDs cannot be cancelled");
  await f.render([{ code: "int polling;", target: "agent:compiler" }]); await f.advance();
  await f.advance(10000); await f.respond(1, remote("queued"));
  await f.advance(25001);
  assert.equal((await f.snapshot()).status, "unavailable", "polling shares submission's deadline");
  assert.equal((await f.requests())[2].aborted, true);
  assert.deepEqual((await f.requests())[3].body, { job_id: "fixture-job" });
}));

test("late remote submission after a target switch only cancels its job, without polling or fallback", t => run(t, async f => {
  await f.render([{ target: "agent:compiler" }]); await f.advance();
  await f.render([{ code: "" }]);
  await f.respond(0, remote("queued"));
  assert.deepEqual((await f.requests()).map(request => request.url.split(".invalid")[1]), ["/ebpf/check/remote", "/ebpf/check/remote/cancel"]);
  assert.deepEqual(await f.snapshot(), { diagnostics: [], status: "idle" });
}));

test("completed remote compiler results retain issue/note mapping and never trigger a local run", t => run(t, async f => {
  await f.render([{ target: "agent:compiler" }]); await f.advance();
  await f.respond(0, remote("queued"));
  await f.respond(1, remote("failed", report("clang rejected source", false)));
  const result = await f.snapshot();
  assert.equal(result.status, "issues"); assert.equal(result.diagnostics[0].severity, "info");
  assert.equal((await f.requests()).length, 2);
  assert.equal((await f.requests())[0].body.agent_id, "compiler");
}));

test("same-editor cache reuse stops at the exact eight-second expiry", t => run(t, async f => {
  await f.render(); await f.advance(); await f.respond(0, report());
  await f.render([{ code: "" }]); await f.advance(7999); await f.render();
  assert.equal((await f.snapshot()).status, "passed");
  assert.equal((await f.requests()).length, 1);
  await f.advance(1); await f.render([{ code: "" }]); await f.render();
  assert.equal((await f.snapshot()).status, "idle");
  await f.advance(); assert.equal((await f.requests()).length, 2);
}));

test("cache retains at most 24 entries per editor", t => run(t, async f => {
  await f.page.evaluate(() => { Date.now = () => 123456; });
  for (let index = 0; index < 25; index++) {
    await f.render([{ code: `int source_${index};` }]); await f.advance();
    await f.respond(index, report(`source ${index}`));
  }
  await f.render([{ code: "int source_1;" }]);
  assert.equal((await f.snapshot()).status, "passed", "non-evicted entry remains cached");
  await f.render([{ code: "int source_0;" }]); await f.advance();
  assert.equal((await f.requests()).length, 26, "oldest entry was evicted");
}));

test("rapid edits, blank drafts and pre-debounce unmounts dispatch no obsolete checks", t => run(t, async f => {
  await f.render(); await f.advance(699); await f.render([{ code: "int latest;" }]);
  await f.advance(699); assert.equal((await f.requests()).length, 0);
  await f.advance(1); assert.equal((await f.requests())[0].body.code, "int latest;");
  await f.render([{ code: " " }]); await f.respond(0, report("discarded"));
  assert.deepEqual(await f.snapshot(), { diagnostics: [], status: "idle" });
  await f.render([{ code: "x".repeat(262145) }]); await f.advance(1200);
  assert.equal((await f.requests()).length, 1);
  await f.render(); await f.render([]); await f.advance();
  assert.equal((await f.requests()).length, 1);
}));

test("failed checks are not cached and local requests cannot wait beyond 20 seconds", t => run(t, async f => {
  await f.page.evaluate(() => { window.fixture.honorAbort = true; });
  await f.render(); await f.advance(); await f.advance(20000);
  assert.equal((await f.snapshot()).status, "unavailable");
  await f.render([{ code: "" }]); await f.render(); await f.advance();
  assert.equal((await f.requests()).length, 2);
  await f.respond(1, report()); assert.equal((await f.snapshot()).status, "passed");
}));
