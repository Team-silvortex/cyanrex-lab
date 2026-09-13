import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  bundle = await buildFixture(new URL("./fixtures/breakpointLifecycle.mjs", import.meta.url));
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const event = (line = 2, session = "session-a", extra = {}) => ({ username: "fixture", timestamp: "2026-09-13T00:00:00Z",
  source: "module-ebpf", event_type: "ebpf.debug_breakpoint_hit", category: "kernel", severity: "success", color: "green",
  payload: { line, debug_session_id: session }, ...extra });
async function run(callback) {
  const f = await setupFixture(browser, bundle); f.act = fn => f.page.evaluate(fn);
  f.render = (value = {}) => f.page.evaluate(options => fixture.render(options), value);
  f.message = value => f.page.evaluate(row => fixture.message(row), value);
  f.open = () => f.act(() => fixture.open());
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}
async function ready(f, rows = []) { await f.render(); await f.open(); await f.respond(0, rows); }

test("a new debug session never renders the previous session's hits, even before effects", () => run(async f => {
  await ready(f, [event()]); await f.act(() => { fixture.history = []; }); await f.render({ session: "session-b" });
  const renders = await f.act(() => fixture.history);
  assert.ok(renders.length > 0); assert.ok(renders.every(row => row.hits.length === 0));
}));

test("clearing the debug session immediately removes old hits and gap state", () => run(async f => {
  await ready(f, [event()]); await f.act(() => { fixture.disconnect(); fixture.history = []; });
  await f.render({ session: null });
  const cleared = (await f.act(() => fixture.history)).filter(row => row.session === null);
  assert.ok(cleared.length > 0); assert.ok(cleared.every(row => row.hits.length === 0 && !row.streamGap));
}));

test("an Engine switch cannot render hits from a same-named session on the previous Engine", () => run(async f => {
  await ready(f, [event()]); await f.act(() => { fixture.history = []; }); await f.render({ engine: "https://engine-b.invalid" });
  assert.ok((await f.act(() => fixture.history)).every(row => row.hits.length === 0));
}));

test("snapshot hits accept only positive integral instrumented line numbers", () => run(async f => {
  await ready(f, [0, -1, 2.5, 999, 2].map(line => event(line)));
  assert.deepEqual((await f.snapshot()).hits.map(row => row.line), [2]);
}));

test("live hits reject other categories and non-instrumented lines", () => run(async f => {
  await f.render({ lines: [2] }); await f.open(); await f.respond(0, []);
  await f.message(event(2, "session-a", { category: "platform" })); await f.message(event(3)); await f.message(event(2));
  assert.deepEqual((await f.snapshot()).hits.map(row => row.line), [2]);
}));

test("changing the instrumented set invalidates the old subscription and pending snapshot", () => run(async f => {
  await f.render(); await f.open(); await f.render({ lines: [3] });
  assert.equal((await f.requests())[0].aborted, true);
  await f.respond(0, [event()]); assert.deepEqual((await f.snapshot()).hits, []);
}));

test("event recovery snapshots are private credentialed reads that reject redirects", () => run(async f => {
  await f.render(); await f.open(); const request = (await f.requests())[0];
  assert.equal(request.credentials, "include"); assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error");
}));

test("malformed live frames leave a visible gap without manufacturing hits or reconnecting", () => run(async f => {
  await ready(f, [event()]); const sockets = await f.act(() => fixture.sockets.length);
  await f.message("{broken"); assert.equal((await f.snapshot()).streamGap, true);
  assert.deepEqual((await f.snapshot()).hits.map(row => row.line), [2]);
  assert.equal(await f.act(() => fixture.sockets.length), sockets);
}));

test("replacing the editor removes decorations from the old owning model", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => fixture.attach("old")); await f.act(() => fixture.key("old"));
  assert.equal((await f.act(() => fixture.decorations("old"))).length, 1);
  await f.act(() => fixture.attach("new")); assert.deepEqual(await f.act(() => fixture.decorations("old")), []);
}));

test("late keyboard callbacks from a replaced editor cannot toggle the current editor's breakpoints", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => { fixture.attach("old"); fixture.attach("new"); });
  await f.act(() => fixture.key("old", true)); assert.deepEqual((await f.snapshot()).breakpoints, []);
}));

test("out-of-model and fractional hit lines never create editor decorations", () => run(async f => {
  await f.render({ mode: "editor", hit: 999 }); await f.act(() => fixture.attach("owned"));
  assert.deepEqual(await f.act(() => fixture.decorations("owned")), []);
  await f.render({ hit: 1.5 }); assert.deepEqual(await f.act(() => fixture.decorations("owned")), []);
}));

test("model replacement retires the previous model's decorations and clamps current breakpoints", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => { fixture.attach("owned"); fixture.key("owned"); });
  await f.act(() => fixture.swapModel("owned", "single line"));
  assert.deepEqual(await f.act(() => fixture.decorations("owned")), []);
  assert.deepEqual((await f.snapshot()).breakpoints, []);
}));

test("editor disposal unregisters keyboard callbacks before the controller unmounts", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => fixture.attach("owned"));
  await f.act(() => fixture.editors.owned.dispose());
  assert.equal(await f.act(() => fixture.editors.owned.keys.active.size), 0);
  await f.act(() => fixture.key("owned", true)); assert.deepEqual((await f.snapshot()).breakpoints, []);
}));

test("late disposal of a previous model cannot clear the replacement model's decorations", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => { fixture.attach("owned"); fixture.key("owned"); });
  await f.act(() => fixture.swapModel("owned", "a\nb\nc"));
  assert.equal((await f.act(() => fixture.decorations("replacement"))).length, 1);
  await f.act(() => fixture.models.owned.disposal.all[0]());
  assert.equal((await f.act(() => fixture.decorations("replacement"))).length, 1);
}));

test("recovery retains its gap notice, filters sessions, and ignores old socket callbacks", () => run(async f => {
  await ready(f, [event()]); await f.act(() => fixture.disconnect()); await f.advance(750);
  await f.open(); await f.respond(1, [event(2, "other-session"), event(3)]);
  assert.deepEqual((await f.snapshot()).hits.map(row => row.line), [3]);
  assert.equal((await f.snapshot()).connection, "open"); assert.equal((await f.snapshot()).streamGap, true);
  await f.page.evaluate(row => fixture.message(row, 1), event(2));
  assert.deepEqual((await f.snapshot()).hits.map(row => row.line), [3]);
}));

test("breakpoint view retains at most fifty hits while preserving repeated occurrences", () => run(async f => {
  const rows = Array.from({ length: 80 }, (_, index) => event(2, "session-a", { timestamp: new Date(1700000000000 + index).toISOString() }));
  await ready(f, rows.slice(0, 60)); assert.equal((await f.snapshot()).hits.length, 50);
  for (const row of rows.slice(60)) await f.message(row);
  const hits = (await f.snapshot()).hits;
  assert.equal(hits.length, 50); assert.equal(hits[0].timestamp, rows[30].timestamp); assert.equal(hits.at(-1).timestamp, rows[79].timestamp);
}));

test("empty instrumented sets do not subscribe and canonical-equivalent sets do not reconnect", () => run(async f => {
  await f.render({ lines: [] }); assert.equal(await f.act(() => fixture.sockets.length), 0);
  assert.equal((await f.snapshot()).connection, "closed");
  await f.render({ lines: [3, 2, 2] }); const count = await f.act(() => fixture.sockets.length);
  await f.render({ lines: [2, 3] }); assert.equal(await f.act(() => fixture.sockets.length), count);
}));

test("snapshot timeout recovers but leaving the session cancels all retries and late results", () => run(async f => {
  await f.act(() => { fixture.honorAbort = true; }); await f.render(); await f.open(); await f.advance(10000);
  assert.equal((await f.requests())[0].aborted, true); assert.equal((await f.snapshot()).connection, "recovering");
  const count = await f.act(() => fixture.sockets.length); await f.render({ session: null }); await f.advance(60000);
  await f.respond(0, [event()]); assert.deepEqual((await f.snapshot()).hits, []);
  assert.equal(await f.act(() => fixture.sockets.length), count);
  assert.equal(await f.act(() => fixture.sockets.filter(socket => !socket.closed).length), 0);
}));

test("current-editor F9, gutter toggles and clearing keep valid hit decoration independent", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => { fixture.attach("owned"); fixture.key("owned"); });
  assert.deepEqual((await f.snapshot()).breakpoints, [2]); await f.render({ hit: 2 });
  assert.equal((await f.act(() => fixture.decorations("owned")))[0].className, "cyanrex-breakpoint-hit-line");
  await f.act(() => fixture.controller.clearDebugBreakpoints()); assert.deepEqual((await f.snapshot()).breakpoints, []);
  assert.equal((await f.act(() => fixture.decorations("owned")))[0].line, 2);
  await f.render({ hit: null }); assert.deepEqual(await f.act(() => fixture.decorations("owned")), []);
  await f.act(() => fixture.editors.owned.mouse.fire({ target: { type: 2, position: { lineNumber: 3 } }, event: { preventDefault() {} } }));
  assert.deepEqual((await f.snapshot()).breakpoints, [3]);
}));

test("controller unmount clears its owning model even if the external editor reference changed", () => run(async f => {
  await f.render({ mode: "editor" }); await f.act(() => { fixture.attach("owned"); fixture.key("owned"); fixture.editorRef.current = null; });
  await f.render(false); assert.deepEqual(await f.act(() => fixture.decorations("owned")), []);
  assert.equal(await f.act(() => fixture.editors.owned.keys.active.size), 0);
}));
