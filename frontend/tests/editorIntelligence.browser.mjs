import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  bundle = await buildFixture(new URL("./fixtures/editorIntelligence.mjs", import.meta.url));
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const semantic = label => ({ ok: true, message: "fixture", items: [{ label, insert_text: label, detail: "fixture", kind: "field" }] });
const checked = { ok: true, message: "old check", diagnostics: [], stdout: "", stderr: "" };
const header = id => ({ id, include_hint: `<${id}.h>`, local_path: `/fixture/${id}.h`, downloaded: true });
async function run(callback) {
  const f = await setupFixture(browser, bundle);
  f.act = fn => f.page.evaluate(fn);
  f.labels = id => f.page.evaluate(key => window.fixture.results[key]?.suggestions.map(item => item.label), id);
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); }
  finally { await f.close(); }
}

test("completion providers never send another editor's model to their Engine", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.makeModel("foreign"); fixture.complete("owned", "wrong", "foreign"); });
  assert.equal((await f.requests()).length, 0);
  assert.deepEqual(await f.labels("wrong"), []);
}));

test("disposing a provider aborts pending completions and discards late items", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.complete("owned"); fixture.handles.owned.dispose(); });
  assert.equal((await f.requests())[0].aborted, true);
  await f.respond(0, semantic("late"));
  assert.deepEqual(await f.labels("owned"), []);
}));

test("model edits and replacements invalidate completion callbacks even without token cancellation", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.complete("owned"); fixture.models.owned.change("int changed;"); });
  await f.respond(0, semantic("old_version")); assert.deepEqual(await f.labels("owned"), []);
  await f.act(() => { fixture.complete("owned", "replaced"); fixture.editors.owned.setModel(fixture.makeModel("new")); });
  await f.respond(1, semantic("old_model")); assert.deepEqual(await f.labels("replaced"), []);
}));

test("completion cache cannot survive provider/header-context replacement", () => run(async f => {
  await f.act(() => { fixture.register("owned", "header-a"); fixture.complete("owned", "first"); });
  await f.respond(0, semantic("header_a_field"));
  await f.act(() => { fixture.handles.owned.dispose(); fixture.register("owned", "header-b"); fixture.complete("owned", "second"); });
  assert.equal((await f.requests()).length, 2);
  await f.respond(1, semantic("header_b_field")); assert.ok((await f.labels("second")).includes("header_b_field"));
}));

test("pre-cancelled completion tokens never return cached semantic suggestions", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.complete("owned", "first"); });
  await f.respond(0, semantic("cached"));
  await f.act(() => { fixture.complete("owned", "cancelled", "owned", true); });
  assert.deepEqual(await f.labels("cancelled"), []);
  assert.equal((await f.requests()).length, 1);
}));

test("separate completion providers do not share cancellation ownership", () => run(async f => {
  await f.act(() => { fixture.honorAbort = true; fixture.register("first"); fixture.register("second"); fixture.complete("first"); fixture.complete("second"); });
  const survivor = (await f.requests()).length - 1;
  await f.act(() => fixture.tokens.first.cancel());
  assert.equal((await f.requests())[survivor].aborted, false);
  await f.respond(survivor, semantic("survivor")); assert.ok((await f.labels("second")).includes("survivor"));
}));

test("completion network failures retain snippets without unhandled promise rejections", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.complete("owned"); });
  await f.reject(0); assert.ok((await f.labels("owned")).includes("SEC xdp"));
}));

test("stalled completion is aborted after ten seconds while static snippets remain available", () => run(async f => {
  await f.act(() => { fixture.honorAbort = true; fixture.register("owned"); fixture.complete("owned"); });
  await f.advance(10000);
  assert.equal((await f.requests())[0].aborted, true);
  assert.ok((await f.labels("owned")).includes("SEC xdp"));
}));

test("section snippets insert actual line breaks, not literal backslash-n text", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.complete("owned", "static", "owned", false, 1); });
  const snippets = await f.act(() => fixture.results.static.suggestions.filter(item => item.label.startsWith("SEC ")));
  assert.equal(snippets.length, 3);
  for (const item of snippets) { assert.ok(item.insertText.includes("\n"), item.label); assert.equal(item.insertText.includes("\\n"), false); }
  assert.equal((await f.requests()).length, 0);
}));

test("manual header checks discard late results after source changes", () => run(async f => {
  await f.render(); await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.act(() => fixture.changeCode("int changed;")); await f.respond(0, checked);
  assert.equal((await f.snapshot()).check.status, "idle"); assert.equal((await f.requests())[0].aborted, true);
}));

test("duplicate manual-check calls dispatch only once", () => run(async f => {
  await f.render(); await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); void fixture.controller.runHeaderInjectionSelfCheck(); });
  assert.equal((await f.requests()).length, 1);
  await f.respond(0, checked); assert.equal((await f.snapshot()).check.status, "passed");
}));

test("manual header checks abort on unmount and have a complete-request deadline", () => run(async f => {
  await f.render(); await f.act(() => { fixture.honorAbort = true; void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.advance(20000); assert.equal((await f.snapshot()).check.status, "error");
  await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.render(false); assert.equal((await f.requests()).at(-1).aborted, true);
}));

test("manual HTTP failures are infrastructure errors rather than compiler issues", () => run(async f => {
  await f.render(); await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.respond(0, { ...checked, ok: false, message: "service unavailable" }, 503);
  assert.equal((await f.snapshot()).check.status, "error");
}));

test("overlapping metadata refreshes cannot replace a newer header selection", () => run(async f => {
  await f.render(); await f.act(() => { fixture.holdMetadata = true; void fixture.controller.refreshInjectedMetadata(); void fixture.controller.refreshInjectedMetadata(); });
  await f.respond(1, { selected_headers: [header("new")] });
  await f.respond(0, { selected_headers: [header("old")] });
  assert.deepEqual((await f.snapshot()).metadata.map(item => item.id), ["new"]);
}));

test("metadata refresh failure is visible without silently claiming an empty selection", () => run(async f => {
  await f.act(() => { fixture.metadata = [{ id: "kept", include_hint: "<kept.h>", local_path: "/fixture/kept.h", downloaded: true }]; });
  await f.render(); await f.act(() => { fixture.holdMetadata = true; void fixture.controller.refreshInjectedMetadata(); });
  await f.respond(0, { message: "forbidden" }, 403);
  assert.ok((await f.snapshot()).metadataError);
  assert.deepEqual((await f.snapshot()).metadata.map(item => item.id), ["kept"]);
}));

test("an unrelated first Monaco model never receives this editor's diagnostic markers", () => run(async f => {
  await f.render(); await f.act(() => { fixture.mountEditor(); fixture.markerCalls.length = 0; fixture.controller.onEditorChange("int changed;"); });
  const calls = await f.act(() => fixture.markerCalls);
  assert.ok(calls.length > 0); assert.ok(calls.every(call => call.id === "owned"));
}));

test("malformed metadata rows are a visible failure without crashing the editor", () => run(async f => {
  await f.render(); await f.act(() => { fixture.holdMetadata = true; void fixture.controller.refreshInjectedMetadata(); });
  await f.respond(0, { selected_headers: [null] }); await f.advance(1);
  assert.deepEqual(f.errors, []); assert.ok((await f.snapshot()).metadataError);
}));

test("owner disposal unregisters every language provider", () => run(async f => {
  await f.act(() => { fixture.register("owned"); fixture.editors.owned.dispose(); });
  assert.equal(await f.act(() => fixture.registrations.filter(item => !item.disposed).length), 0);
}));

test("refreshing unchanged headers invalidates pending checks and the mounted completion provider", () => run(async f => {
  await f.render();
  await f.act(() => {
    fixture.mountEditor();
    fixture.providers.owned = fixture.registrations.findLast(item => item.kind === "CompletionItem").provider;
    fixture.complete("owned", "old"); void fixture.controller.runHeaderInjectionSelfCheck();
  });
  await f.act(() => { fixture.holdMetadata = true; void fixture.controller.refreshInjectedMetadata(); });
  const requests = await f.requests(); assert.equal(requests[0].aborted, true); assert.equal(requests[1].aborted, true);
  await f.respond(0, semantic("obsolete")); await f.respond(1, checked);
  assert.deepEqual(await f.labels("old"), []); assert.equal((await f.snapshot()).check.status, "idle");
  await f.respond(2, { selected_headers: [] });
  await f.act(() => {
    fixture.providers.owned = fixture.registrations.findLast(item => item.kind === "CompletionItem").provider;
    fixture.complete("owned", "fresh");
  });
  await f.respond(3, semantic("fresh")); assert.ok((await f.labels("fresh")).includes("fresh"));
  assert.equal(await f.act(() => fixture.registrations.filter(item => !item.disposed).length), 6);
}));

test("metadata deadlines end loading without losing the last selection and unmount aborts refresh", () => run(async f => {
  await f.act(() => { fixture.metadata = [{ id: "kept", include_hint: "<kept.h>", local_path: "/fixture/kept.h", downloaded: true }]; });
  await f.render();
  await f.act(() => { fixture.honorAbort = true; fixture.holdMetadata = true; void fixture.controller.refreshInjectedMetadata(); });
  await f.advance(10000);
  assert.equal((await f.requests())[0].aborted, true);
  const value = await f.snapshot(); assert.equal(value.metadataLoading, false); assert.ok(value.metadataError);
  assert.equal(value.metadata[0].id, "kept");
  await f.act(() => { void fixture.controller.refreshInjectedMetadata(); });
  await f.render(false); assert.equal((await f.requests()).at(-1).aborted, true);
}));

test("late manual-check errors cannot erase a newer successful result", () => run(async f => {
  await f.render(); await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.act(() => fixture.changeCode("int changed;"));
  await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  await f.respond(1, { ...checked, message: "current result" }); await f.reject(0);
  const value = (await f.snapshot()).check; assert.equal(value.status, "passed"); assert.equal(value.message, "current result");
}));

test("normal manual compiler rejection remains issues and sends only source to the local check", () => run(async f => {
  await f.render(); await f.act(() => { void fixture.controller.runHeaderInjectionSelfCheck(); });
  const request = (await f.requests())[0]; assert.ok(request.url.endsWith("/ebpf/check"));
  assert.deepEqual(Object.keys(request.body), ["code"]); assert.equal(request.cache, "no-store");
  await f.respond(0, { ...checked, ok: false, diagnostics: [{ line: 1, column: 1, severity: "error", message: "fixture" }] });
  const value = (await f.snapshot()).check; assert.equal(value.status, "issues"); assert.equal(value.diagnostics, 1);
  assert.equal((await f.requests()).length, 1);
}));
