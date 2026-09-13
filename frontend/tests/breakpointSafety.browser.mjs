import assert from "node:assert/strict";
import test from "node:test";
import { baseUrl, target, setup } from "./helpers/actionSafetyBrowser.mjs";

const event = (session = "session-a", line = 2, index = 0) => ({ username: "student", timestamp: new Date(1700000000000 + index).toISOString(),
  source: "module-ebpf", event_type: "ebpf.debug_breakpoint_hit", category: "kernel", severity: "success", color: "green",
  payload: { debug_session_id: session, line } });
const result = session => ({ success: true, stage: "run", message: "fixture attached", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "", pin_path: target,
  debug: { mode: "kernel-trace", session_id: session, requested_lines: [2], instrumented_lines: [2], rejected: [] } });
const dialog = page => page.getByRole("dialog", { name: "确认操作", exact: true });
const badge = page => page.locator(".editor-debug .event-tag");
const hits = page => page.evaluate(() => window.monaco.editor.getEditors()[0].getModel().getAllDecorations()
  .filter(item => item.options.className === "cyanrex-breakpoint-hit-line").map(item => item.range.startLineNumber));
async function run(f) {
  await f.page.getByRole("button", { name: "编译并运行", exact: true }).click();
  await dialog(f.page).getByRole("button", { name: "确认编译并运行", exact: true }).click();
  await dialog(f.page).waitFor({ state: "detached" });
}
async function ready(f) {
  f.state.runResult = result("session-a"); f.state.events = [event()];
  await f.page.goto(`${baseUrl}/ebpf`); await f.page.getByText(/clang: passed/).first().waitFor();
}

test("real Monaco binds hit decorations to the declared session and retires them on edit and cleanup", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await ready(f);
    await f.page.evaluate(() => { const editor = window.monaco.editor.getEditors()[0]; editor.setPosition({ lineNumber: 2, column: 1 }); editor.focus(); });
    await f.page.keyboard.press("F9");
    await f.page.waitForFunction(() => JSON.parse(sessionStorage.getItem("ebpf_debug_breakpoints_v1") || "[]").includes(2));
    f.state.events = [event("other"), event("session-a", 0), event("session-a", 999), event()];
    await run(f); await badge(f.page).filter({ hasText: "L2 · 1" }).waitFor();
    assert.deepEqual(f.writes[0].body.debug_breakpoints, [2]); assert.deepEqual(await hits(f.page), [2]);
    await f.page.evaluate(() => window.monaco.editor.getEditors()[0].setValue("// changed draft\nint changed(void) { return 2; }"));
    await f.page.locator(".result-panel").getByRole("status").filter({ hasText: "旧结果已隐藏" }).waitFor();
    await f.page.waitForFunction(() => window.monaco.editor.getEditors()[0].getModel().getAllDecorations()
      .every(item => item.options.className !== "cyanrex-breakpoint-hit-line"));
    assert.equal(await badge(f.page).count(), 0); assert.equal(f.writes.length, 1);
    assert.equal(await f.page.evaluate(() => window.fixtureSockets.filter(socket => !socket.closed).length), 0);
    f.state.runResult = result("session-b"); f.state.events = [event(), event("session-b")];
    await run(f); await badge(f.page).filter({ hasText: "L2 · 1" }).waitFor();
    f.state.attachments = [];
    await f.page.locator(".attachments-panel").getByRole("button", { name: "卸载", exact: true }).first().click();
    await dialog(f.page).getByRole("button", { name: /^确认/ }).click(); await dialog(f.page).waitFor({ state: "detached" });
    await badge(f.page).waitFor({ state: "detached" }); assert.deepEqual(await hits(f.page), []);
    assert.deepEqual(f.writes.map(item => item.path), ["/ebpf/run", "/ebpf/run", "/ebpf/detach"]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("breakpoint recovery keeps its gap warning and rejects obsolete sockets without loading again", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await ready(f); await run(f); await badge(f.page).filter({ hasText: "L2 · 1" }).waitFor();
    await f.page.evaluate(() => window.fixtureSockets.at(-1).onmessage({ data: "{broken" }));
    await f.page.getByRole("status").filter({ hasText: "部分事件可能缺失" }).waitFor();
    assert.equal(f.state.eventReads.length, 1);
    f.state.events = [event(), event("session-a", 2, 1), event("other", 2, 2)];
    await f.page.evaluate(() => { const socket = window.fixtureSockets.at(-1); socket.closed = true; socket.onclose({ code: 1013 }); });
    await badge(f.page).filter({ hasText: "L2 · 2" }).waitFor();
    assert.equal(f.state.eventReads.length, 2);
    await f.page.evaluate(row => window.fixtureSockets[0].onmessage({ data: JSON.stringify(row) }), event("session-a", 2, 99));
    assert.equal(await badge(f.page).textContent(), "L2 · 2");
    assert.equal(await f.page.getByRole("status").filter({ hasText: "部分事件可能缺失" }).count(), 1);
    assert.equal(await f.page.evaluate(() => window.fixtureSockets.filter(socket => !socket.closed).length), 1);
    await f.page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    assert.equal(f.writes.length, 1); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("forbidden breakpoint snapshots stop retries and never masquerade as live observation", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await ready(f); f.state.eventsStatus = 403; await run(f);
    await f.page.getByRole("status").filter({ hasText: "已断开" }).waitFor();
    assert.equal(await badge(f.page).count(), 0); assert.equal(f.state.eventReads.length, 1);
    await f.page.clock.install(); await f.page.clock.fastForward(60000);
    assert.equal(f.state.eventReads.length, 1); assert.equal(f.writes.length, 1);
    assert.equal(await f.page.evaluate(() => window.fixtureSockets.filter(socket => !socket.closed).length), 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
