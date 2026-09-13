import assert from "node:assert/strict";
import test from "node:test";

import { baseUrl, draft, target, setup } from "./helpers/actionSafetyBrowser.mjs";

const dialog = page => page.getByRole("dialog", { name: "确认操作", exact: true });
const approve = page => dialog(page).getByRole("button", { name: /^确认/ });

test("teachers manage modules, deployment settings and terminal without switching role, keeping confirmations", { timeout: 45000 }, async () => {
  const f = await setup("teacher");
  try {
    await f.page.goto(`${baseUrl}/modules`);
    await f.page.getByTestId("workspace-role").waitFor();
    assert.equal((await f.page.getByTestId("workspace-role").textContent()).trim(), "教师 · 教学与部署管理");
    await f.page.getByRole("button", { name: "删除", exact: true }).first().click();
    await dialog(f.page).waitFor();
    assert.equal(f.writes.length, 0);
    await f.page.keyboard.press("Escape");
    await f.page.locator('nav a[href="/settings"]').click();
    await f.page.getByRole("heading", { name: "Runner Agent 运维", exact: true }).waitFor();
    await f.page.getByRole("button", { name: "保存设置", exact: true }).click();
    await dialog(f.page).waitFor();
    assert.equal(f.writes.length, 0);
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes.map(item => item.path), ["/settings/events", "/settings/compiler"]);
    if (process.env.CYANREX_AUTHORITY_SCREENSHOT) await f.page.screenshot({ path: process.env.CYANREX_AUTHORITY_SCREENSHOT });
    await f.page.locator('nav a[href="/terminal"]').click();
    await f.page.getByRole("button", { name: "执行命令", exact: true }).click();
    await f.page.getByText("done", { exact: false }).first().waitFor();
    assert.equal(f.writes[2].body.commandType, "ListModules");
    assert.equal(f.writes.length, 3);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("students keep experiment access without teacher deployment navigation", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.getByTestId("workspace-role").waitFor();
    assert.equal((await f.page.getByTestId("workspace-role").textContent()).trim(), "学生");
    for (const route of ["/settings", "/terminal", "/modules", "/teaching"]) {
      assert.equal(await f.page.locator(`nav a[href="${route}"]`).count(), 0);
    }
    await f.page.locator(".monaco-editor").waitFor();
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("single deletion focuses cancel, Escape/Enter cancel safely, and failures cannot be double-submitted", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    const saved = f.page.locator("details.workspace-disclosure").filter({ hasText: "已保存脚本" });
    await saved.locator("summary").click();
    const remove = saved.getByRole("button", { name: "删除", exact: true });
    await remove.click();
    await dialog(f.page).waitFor({ timeout: 2000 });
    assert.equal(f.writes.length, 0);
    await dialog(f.page).getByText("My saved script", { exact: false }).waitFor();
    assert.equal(await dialog(f.page).getByRole("button", { name: "取消", exact: true }).evaluate(el => el === document.activeElement), true);
    await f.page.keyboard.press("Escape");
    assert.equal(await remove.evaluate(el => el === document.activeElement), true);
    await remove.click();
    await f.page.keyboard.press("Enter");
    assert.equal(await dialog(f.page).count(), 0);
    assert.equal(f.writes.length, 0);
    let release;
    f.state.hold = () => new Promise(resolve => { release = resolve; });
    f.state.fail = true;
    await remove.click();
    await approve(f.page).dblclick();
    await f.page.waitForFunction(() => document.querySelector('dialog [role="status"]'));
    assert.equal(f.writes.length, 1);
    await f.page.keyboard.press("Escape");
    assert.equal(await dialog(f.page).isVisible(), true, "an in-flight request is not cancelled by Escape");
    release();
    await dialog(f.page).getByRole("alert").waitFor();
    assert.equal(await approve(f.page).count(), 0, "a failed request requires checking status and a fresh confirmation");
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("module lifecycle commands require confirmation but read-only commands do not", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/terminal`);
    const run = f.page.getByRole("button", { name: "执行命令", exact: true });
    await run.click();
    await f.page.getByText("done", { exact: false }).first().waitFor();
    assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0].body.commandType, "ListModules");
    await f.page.locator("select").filter({ has: f.page.locator('option[value="StopModule"]') }).selectOption("StopModule");
    await f.page.locator('input[list="terminal-module-catalog"]').fill("module-a");
    await run.click();
    await dialog(f.page).getByText("module-a", { exact: true }).waitFor();
    await dialog(f.page).getByText("StopModule", { exact: true }).waitFor();
    assert.equal(f.writes.length, 1);
    await f.page.keyboard.press("Escape");
    await run.click();
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes[1].body, { commandType: "StopModule", moduleName: "module-a" });
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("account deletion keeps credential and typed gates, cancels without writes, and reports failure", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/account`);
    const form = f.page.locator("form").last();
    const remove = form.getByRole("button", { name: "删除账号", exact: true });
    await remove.waitFor();
    assert.equal(await remove.isDisabled(), true);
    await form.locator('input[type="password"]').fill("synthetic-test-password");
    await form.locator('input[type="text"]').first().fill("123456");
    await form.locator('input[type="text"]').last().fill("DELETE");
    await remove.click();
    await dialog(f.page).waitFor();
    assert.equal(f.writes.length, 0);
    await f.page.keyboard.press("Escape");
    assert.equal(await form.locator('input[type="password"]').inputValue(), "synthetic-test-password");
    f.state.fail = true;
    await remove.click();
    await approve(f.page).click();
    await dialog(f.page).getByRole("alert").waitFor();
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.writes[0].body, { password: "synthetic-test-password", otp: "123456" });
    assert.ok(f.page.url().endsWith("/account"));
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("job cancellation binds the full job id and settings need explicit review", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/settings`);
    await f.page.getByRole("button", { name: "取消", exact: true }).click();
    await dialog(f.page).getByText("job-a-full-confirmation-id", { exact: true }).waitFor();
    await dialog(f.page).getByText("student", { exact: true }).waitFor();
    assert.equal(f.writes.length, 0);
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes[0].body, { job_id: "job-a-full-confirmation-id" });
    await f.page.getByRole("button", { name: "保存设置", exact: true }).click();
    await dialog(f.page).getByText("500", { exact: true }).waitFor();
    assert.equal(f.writes.length, 1);
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes.slice(1).map(item => item.path), ["/settings/events", "/settings/compiler"]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("confirmation stays usable in every language on mobile and navigation discards an unconfirmed action", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    await f.page.setViewportSize({ width: 320, height: 844 });
    for (const locale of ["en", "es", "ja", "zh-CN"]) {
      await f.page.locator(".menu-toggle").click();
      await f.page.locator(".sidebar-footer select").selectOption(locale);
      await f.page.keyboard.press("Escape");
      await f.page.locator(".editor-actions .button-primary").click();
      const modal = f.page.getByRole("dialog");
      await modal.waitFor();
      const box = await modal.boundingBox();
      assert.ok(box.x >= 0 && box.x + box.width <= 320 && box.y >= 0 && box.y + box.height <= 844);
      assert.equal(await modal.evaluate(el => el.scrollWidth <= el.clientWidth), true);
      await f.page.mouse.click(2, 2);
      assert.equal(await modal.isVisible(), true, "backdrop clicks must not submit or dismiss");
      if (process.env.CYANREX_UI_SCREENSHOT && locale === "zh-CN") await f.page.screenshot({ path: process.env.CYANREX_UI_SCREENSHOT });
      await f.page.keyboard.press("Escape");
    }
    await f.page.locator(".editor-actions .button-primary").click();
    await dialog(f.page).waitFor();
    await f.page.evaluate(() => window.next.router.push("/ebpf?lab=01-first-program"));
    await f.page.waitForFunction(() => !document.querySelector("dialog"));
    assert.equal(f.writes.length, 0);
    assert.equal(await f.code(), draft);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("detach-all requires a typed phrase and single detach can never fall back to all", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    const panel = f.page.locator(".attachments-panel");
    await panel.getByText("program-a", { exact: true }).waitFor();
    assert.equal(await panel.getByRole("button", { name: "卸载", exact: true }).first().isDisabled(), true);
    await panel.getByRole("button", { name: "全部卸载", exact: true }).click();
    await dialog(f.page).getByText(target, { exact: true }).waitFor();
    assert.equal(await approve(f.page).isDisabled(), true);
    await dialog(f.page).getByLabel("确认词", { exact: true }).fill("detach");
    assert.equal(await approve(f.page).isDisabled(), true);
    await dialog(f.page).getByLabel("确认词", { exact: true }).fill("DETACH");
    await f.page.keyboard.press("Enter");
    assert.equal(f.writes.length, 0, "typing the phrase and Enter must not submit implicitly");
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes[0].body, { pin_path: null });
    await panel.getByRole("button", { name: "卸载", exact: true }).last().click();
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    assert.deepEqual(f.writes[1].body, { pin_path: target });
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("template/source replacement and kernel runs require separate explicit decisions", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    for (let index = 0; index < 2; index++) {
      const chooser = f.page.waitForEvent("filechooser");
      await f.page.getByRole("button", { name: "导入文件", exact: true }).click();
      await (await chooser).setFiles({ name: "same-file.c", mimeType: "text/plain", buffer: Buffer.from("// Imported draft") });
      await dialog(f.page).getByText("same-file.c", { exact: true }).waitFor();
      await f.page.keyboard.press("Escape");
      assert.equal(await f.code(), draft);
    }
    await f.page.getByLabel("模板", { exact: true }).selectOption("template-a");
    await dialog(f.page).waitFor();
    assert.equal(await f.code(), draft);
    await f.page.keyboard.press("Escape");
    assert.equal(await f.code(), draft);
    assert.equal(await f.page.getByLabel("模板", { exact: true }).inputValue(), "");
    await f.page.getByLabel("模板", { exact: true }).selectOption("template-a");
    await approve(f.page).click();
    await f.page.waitForFunction(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === "// Template source");
    assert.equal(f.writes.length, 0);
    await f.page.getByRole("button", { name: "编译并运行", exact: true }).click();
    await dialog(f.page).getByText("bpftool", { exact: true }).waitFor();
    assert.equal(f.writes.length, 0);
    await approve(f.page).click();
    await f.page.getByText("fixture compilation", { exact: false }).first().waitFor();
    assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0].body.code, "// Template source");
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("remote diagnostics require acknowledging source transfer before switching agents", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    const backend = f.page.getByLabel("内联诊断后端", { exact: true });
    await backend.selectOption("agent:compiler-agent-a");
    await dialog(f.page).getByText(/当前源码和后续修改/).waitFor();
    await f.page.keyboard.press("Escape");
    assert.equal(await backend.inputValue(), "local");
    assert.equal(f.writes.length, 0);
    await backend.selectOption("agent:compiler-agent-a");
    const submitted = f.page.waitForResponse(response => response.url().endsWith("/ebpf/check/remote") && response.request().method() === "POST");
    await approve(f.page).click();
    await submitted;
    const request = f.writes.find(item => item.path === "/ebpf/check/remote");
    assert.equal(request.body.agent_id, "compiler-agent-a");
    assert.equal(request.body.code, draft);
    assert.equal(f.writes.some(item => item.path === "/ebpf/run"), false);
    await backend.selectOption("local");
    assert.equal(await dialog(f.page).count(), 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("remote cancellation and expiry show unavailable in the real editor without running or replacing its draft", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    const backend = f.page.getByLabel("内联诊断后端", { exact: true });
    for (const state of ["cancelled", "expired"]) {
      f.state.remoteState = state;
      await backend.selectOption("agent:compiler-agent-a");
      await approve(f.page).click();
      await f.page.getByText(/clang: unavailable/).first().waitFor();
      assert.equal(await f.page.evaluate(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1"))), draft);
      await backend.selectOption("local");
      await f.page.getByText(/clang: passed/).first().waitFor();
    }
    assert.deepEqual(f.writes.map(item => item.path), ["/ebpf/check/remote", "/ebpf/check/remote"]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("a confirmed file import cannot replace a different lab's draft after navigation", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    await f.page.evaluate(() => {
      File.prototype.text = function () {
        return new Promise(resolve => { window.releaseSafetyUpload = () => resolve("// Late file source must be discarded"); });
      };
    });
    const chooser = f.page.waitForEvent("filechooser");
    await f.page.getByRole("button", { name: "导入文件", exact: true }).click();
    await (await chooser).setFiles({ name: "slow-file.c", mimeType: "text/plain", buffer: Buffer.from("// Late file") });
    await approve(f.page).click();
    await f.page.waitForFunction(() => typeof window.releaseSafetyUpload === "function");
    await f.page.evaluate(() => window.next.router.push("/ebpf?lab=01-first-program"));
    await f.page.waitForFunction(() => !document.querySelector("dialog"));
    await f.page.evaluate(async () => {
      window.releaseSafetyUpload();
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    });
    assert.equal(await f.code(), draft);
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("entering a lab preserves the draft until its template is explicitly loaded", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.labs = [{ lab: { id: "01-first-program", title: "Fixture lab", template_id: "template-a", doc_slug: "labs/01-first-program" }, status: "in_progress", attempts: 1 }];
    await f.page.goto(`${baseUrl}/ebpf?lab=01-first-program`);
    await f.page.getByText("Fixture lab", { exact: false }).waitFor();
    await f.page.getByLabel("模板", { exact: true }).locator('option[value="template-a"]').waitFor({ state: "attached" });
    await f.page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert.equal(await f.code(), draft);
    const load = f.page.getByRole("button", { name: "加载实验模板", exact: true });
    await load.click();
    await f.page.keyboard.press("Escape");
    assert.equal(await f.code(), draft);
    await load.click();
    await approve(f.page).click();
    await f.page.waitForFunction(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === "// Template source");
    assert.equal(await f.page.getByLabel("模板", { exact: true }).inputValue(), "template-a");
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("header batch deletion stops at failure and reports partial completion, never false success", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/modules`);
    await f.page.getByRole("button", { name: "删除已勾选", exact: true }).click();
    await dialog(f.page).getByText("header-a", { exact: true }).waitFor();
    await dialog(f.page).getByText("header-b", { exact: true }).waitFor();
    assert.equal(f.writes.length, 0);
    await dialog(f.page).getByLabel("确认词", { exact: true }).fill("DELETE");
    f.state.failAt = 2;
    await approve(f.page).click();
    await dialog(f.page).getByRole("alert").filter({ hasText: "1/2" }).waitFor();
    assert.equal(f.writes.length, 2);
    assert.deepEqual(f.writes.map(item => item.body), [{ id: "header-a" }, { id: "header-b" }]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("event deletion explains the complete filter scope and freezes the cutoff before confirmation", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/events`);
    await f.page.getByText("fixture.event", { exact: true }).waitFor();
    const before = Date.now();
    await f.page.getByRole("button", { name: "删除筛选结果", exact: true }).click();
    await dialog(f.page).getByText(/不限于当前显示/).waitFor();
    await dialog(f.page).getByLabel("确认词", { exact: true }).fill("DELETE");
    await approve(f.page).click();
    await dialog(f.page).waitFor({ state: "detached" });
    const params = f.writes[0].params;
    assert.ok(Date.parse(params.end) >= before && Date.parse(params.end) <= Date.now());
    assert.equal(params.limit, undefined);
    assert.equal(params.since_minutes, undefined);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("real Monaco accepts semantic suggestions and multiline SEC snippets without executing code", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.completionItems = [{ label: "fixture_symbol", insert_text: "fixture_symbol", detail: "fixture field", kind: "field" }];
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.waitForFunction(() => window.monaco?.editor.getEditors().length);
    // Initial header loading replaces providers; wait for that context's inline check before invoking one.
    await f.page.getByText(/clang: passed/).first().waitFor();
    await f.page.evaluate(() => {
      const editor = window.monaco.editor.getEditors()[0];
      editor.setValue("fixture_"); editor.setPosition({ lineNumber: 1, column: 9 }); editor.focus();
      editor.trigger("test", "editor.action.triggerSuggest", {});
    });
    await f.page.locator(".suggest-widget .monaco-list-row").filter({ hasText: "fixture_symbol" }).waitFor();
    await f.page.keyboard.press("Enter");
    await f.page.waitForFunction(() => window.monaco.editor.getModels()[0].getValue() === "fixture_symbol");
    assert.ok(f.state.completions.some(item => item.code === "fixture_" && item.line === 1 && item.column === 9));
    for (const item of f.state.completions) assert.deepEqual(Object.keys(item).sort(), ["code", "column", "line"]);
    f.state.completionItems = [];
    await f.page.evaluate(() => {
      const editor = window.monaco.editor.getEditors()[0];
      editor.setValue("SEC"); editor.setPosition({ lineNumber: 1, column: 4 }); editor.focus();
      editor.trigger("test", "editor.action.triggerSuggest", {});
    });
    await f.page.locator(".suggest-widget .monaco-list-row").filter({ hasText: /^SEC xdp/ }).dblclick();
    const source = await f.page.evaluate(() => window.monaco.editor.getModels()[0].getValue());
    assert.match(source, /^SEC\("xdp"\)\nint xdp_handler/); assert.equal(source.includes("\\n"), false);
    assert.equal(f.writes.length, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("header refresh errors remain visible while an explicit local self-check still works", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.selectedMetadata = [{ id: "fixture-header", include_hint: "<fixture.h>", local_path: "/fixture/fixture.h", downloaded: true }];
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    const panel = f.page.locator("details.workspace-disclosure").filter({ has: f.page.locator("summary", { hasText: "已注入头文件" }) });
    await panel.locator("summary").first().click();
    await panel.getByText("fixture-header", { exact: true }).waitFor();
    f.state.metadataStatus = 403;
    await panel.getByRole("button", { name: "刷新注入头文件", exact: true }).click();
    await panel.getByRole("alert").filter({ hasText: "无法刷新头文件列表" }).waitFor();
    assert.equal(await panel.getByText("没有已选择头文件元数据。", { exact: true }).count(), 0);
    assert.equal(await panel.getByText("fixture-header", { exact: true }).count(), 1);
    await panel.getByRole("button", { name: "快速自检头文件注入", exact: true }).click();
    await panel.getByText("passed", { exact: true }).waitFor();
    assert.ok(f.state.checks.some(item => item.code === draft));
    for (const item of f.state.checks) assert.deepEqual(Object.keys(item), ["code"]);
    assert.equal(await f.code(), draft); assert.equal(f.writes.length, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
