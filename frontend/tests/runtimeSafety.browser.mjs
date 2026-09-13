import assert from "node:assert/strict";
import test from "node:test";
import { baseUrl, draft, target, setup } from "./helpers/actionSafetyBrowser.mjs";

const dialog = page => page.getByRole("dialog", { name: "确认操作", exact: true });
const approve = page => dialog(page).getByRole("button", { name: /^确认/ });
const loaded = { success: true, stage: "run", message: "fixture loaded", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "", pin_path: target };
const runtime = page => page.getByRole("button", { name: "编译并运行", exact: true });
async function confirmRun(f) { await runtime(f.page).click(); await approve(f.page).click(); await dialog(f.page).waitFor({ state: "detached" }); }

test("real editor hides stale run output without detaching, and confirmed cleanup retires its quick action", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.runResult = loaded;
    await f.page.goto(`${baseUrl}/ebpf`); await f.page.getByText(/clang: passed/).first().waitFor();
    await confirmRun(f); await f.page.locator(".result-panel").getByText(/fixture loaded/).waitFor();
    await f.page.evaluate(() => window.monaco.editor.getEditors()[0].setValue("int changed_source;"));
    await f.page.locator(".result-panel").getByRole("status").filter({ hasText: "旧结果已隐藏" }).waitFor();
    const panel = f.page.locator(".attachments-panel"), quick = panel.getByRole("button", { name: "卸载", exact: true }).first();
    assert.equal(await quick.isDisabled(), true); assert.equal(f.writes.length, 1);
    assert.equal(await panel.getByText("program-a", { exact: true }).count(), 1);
    await confirmRun(f); assert.equal(await quick.isDisabled(), false);
    f.state.attachments = [];
    await quick.click(); await approve(f.page).click(); await dialog(f.page).waitFor({ state: "detached" });
    assert.equal(await quick.isDisabled(), true);
    await panel.getByText("当前没有活跃 pinned eBPF 程序。", { exact: true }).waitFor();
    assert.deepEqual(f.writes.map(item => item.path), ["/ebpf/run", "/ebpf/run", "/ebpf/detach"]);
    assert.deepEqual(f.writes[2].body, { pin_path: target }); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("failed inventory is not empty success and must be refreshed before cleanup controls enable", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.attachmentStatus = 503; f.state.attachments = [];
    await f.page.goto(`${baseUrl}/ebpf`);
    const panel = f.page.locator(".attachments-panel");
    await panel.getByRole("alert").filter({ hasText: "无法刷新挂载状态" }).waitFor();
    assert.equal(await panel.getByText("当前没有活跃 pinned eBPF 程序。", { exact: true }).count(), 0);
    assert.equal(await panel.getByRole("button", { name: "全部卸载", exact: true }).isDisabled(), true);
    f.state.attachmentStatus = 200; f.state.attachments = [{ pin_path: target, source: draft, program_name: "program-a" }];
    await panel.getByRole("button", { name: "刷新挂载列表", exact: true }).click();
    await panel.getByText("program-a", { exact: true }).waitFor();
    assert.equal(await panel.getByRole("alert").count(), 0);
    assert.equal(await panel.getByRole("button", { name: "全部卸载", exact: true }).isDisabled(), false);
    assert.equal(f.writes.length, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("unclean and missing cleanup confirmation remain failures without an automatic retry", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    const panel = f.page.locator(".attachments-panel"); await panel.getByText("program-a", { exact: true }).waitFor();
    for (const clean of [undefined, false]) {
      f.state.detachResult = { ok: true, clean, message: "fixture incomplete cleanup", detached: [target], safety_notes: [] };
      await panel.getByRole("button", { name: "卸载", exact: true }).last().click(); await approve(f.page).click();
      await dialog(f.page).getByRole("alert").filter({ hasText: "尚未确认已清理干净" }).waitFor();
      assert.equal(await approve(f.page).count(), 0); await f.page.keyboard.press("Escape");
    }
    assert.deepEqual(f.writes.map(item => item.body), [{ pin_path: target }, { pin_path: target }]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("a failed run transport stays in the confirmation failure view with an uncertainty notice", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.runStatus = 503; f.state.runResult = { message: "fixture unavailable" };
    await f.page.goto(`${baseUrl}/ebpf`); await runtime(f.page).click(); await approve(f.page).click();
    await dialog(f.page).getByRole("alert").filter({ hasText: "无法确认操作结果" }).waitFor();
    assert.equal(await approve(f.page).count(), 0); assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0].path, "/ebpf/run"); assert.equal(await f.code(), draft);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("navigation discards an in-flight run result without claiming kernel rollback", { timeout: 45000 }, async () => {
  const f = await setup("student"); let release;
  try {
    f.state.runResult = loaded; f.state.hold = () => new Promise(resolve => { release = resolve; });
    await f.page.goto(`${baseUrl}/ebpf`); await runtime(f.page).click(); await approve(f.page).click();
    await dialog(f.page).getByRole("status").waitFor();
    await f.page.evaluate(() => window.next.router.push("/ebpf?lab=02-fixture"));
    await dialog(f.page).waitFor({ state: "detached" }); release(); f.state.hold = null;
    await f.page.locator(".result-panel").getByRole("status").filter({ hasText: "不会自动卸载" }).waitFor();
    assert.equal(await f.page.locator(".result-panel").getByText(/fixture loaded/).count(), 0);
    assert.equal(await runtime(f.page).isDisabled(), false); assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { release?.(); await f.browser.close(); }
});
