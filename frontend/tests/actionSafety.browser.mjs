import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
const draft = "// Unsaved safety draft\nint keep_draft(void) { return 1; }";
const target = "/sys/fs/bpf/safety-fixture/program-a";

async function setup(role = "admin") {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  await context.addInitScript(({ draft, engineUrl }) => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    sessionStorage.setItem("ebpf_code_v1", JSON.stringify(draft));
    const EngineSocket = class { constructor() { setTimeout(() => this.onopen?.({}), 0); } close() {} };
    window.WebSocket = new Proxy(window.WebSocket, { construct(Socket, args) {
      return new URL(args[0], location.href).host === new URL(engineUrl).host
        ? new EngineSocket() : Reflect.construct(Socket, args);
    } });
  }, { draft, engineUrl });
  const page = await context.newPage(), writes = [], errors = [];
  const state = { fail: false, hold: null, headers: ["header-a", "header-b"].map(id => ({ id, name: id, description: "Fixture header", source_url: "fixture", downloaded: true, selected: true, local_path: `/fixture/${id}` })) };
  page.on("pageerror", error => errors.push(error.message));
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === new URL(baseUrl).origin) return route.continue();
    errors.push("Blocked an unmocked external request");
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, async route => {
    const request = route.request(), url = new URL(request.url());
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type" };
    const reply = (json, status = 200) => route.fulfill({ json, status, headers });
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    if (request.method() === "POST" && !["/events/mark-read", "/ebpf/check", "/ebpf/complete"].includes(url.pathname)) {
      writes.push({ path: url.pathname, params: Object.fromEntries(url.searchParams), body: request.postData() ? request.postDataJSON() : null });
      if (state.hold) await state.hold();
      if (state.fail || writes.length === state.failAt) return reply({ ok: false, message: "fixture unavailable" }, 503);
    }
    switch (url.pathname) {
      case "/auth/me": return reply({ authenticated: true, username: role, role });
      case "/events/unread-count": return reply({ unread: 0 });
      case "/events/mark-read": return reply({ ok: true });
      case "/events": return reply([{ username: role, timestamp: "2026-09-08T11:00:00Z", source: "fixture", event_type: "fixture.event", category: "kernel", severity: "error", color: "red", payload: {} }]);
      case "/events/delete": return reply({ ok: true, deleted: 500 });
      case "/learning/labs": return reply(state.labs ?? []);
      case "/ebpf/templates": return reply([{ id: "template-a", name: "Template A", capability: "xdp", code: "// Template source" }]);
      case "/ebpf/attachments/details": return reply({ attachments: [{ pin_path: target, source: draft, program_name: "program-a" }] });
      case "/ebpf/detach": return reply({ ok: true, message: "detached", detached: [target], clean: true });
      case "/modules/c-headers/selected-metadata": return reply({ selected_headers: [] });
      case "/modules/c-headers/catalog": return reply({ headers: state.headers });
      case "/modules/c-headers/delete": return reply({ ok: true, message: "removed" });
      case "/modules/c-headers/select": return reply({ ok: true, message: "selected" });
      case "/ebpf/check/backends": return reply({ local_available: true, agents: [{ agent_id: "compiler-agent-a", isolation: "container", available_slots: 1, max_concurrent: 1 }] });
      case "/ebpf/check": return reply({ ok: true, diagnostics: [], message: "checked", stdout: "", stderr: "" });
      case "/ebpf/check/remote": return reply({ job_id: "remote-check-a", state: "succeeded", result: { ok: true, diagnostics: [], message: "remote checked", stdout: "", stderr: "" } });
      case "/ebpf/check/remote/cancel": return reply({ ok: true });
      case "/ebpf/complete": return reply({ ok: true, items: [], message: "" });
      case "/ebpf/run": return reply({ success: false, stage: "compile", message: "fixture compilation", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "" });
      case "/scripts": return reply([{ id: "script-a", title: "My saved script", script: "// Saved source", updated_at: "2026-09-08T12:00:00Z" }]);
      case "/scripts/delete": return reply({ ok: true, message: "deleted" });
      case "/modules": return reply([{ name: "module-a", status: "running", version: "0.3.5" }]);
      case "/command": return reply({ ok: true, commandType: request.postDataJSON().commandType, message: "done", module: { name: "module-a", status: "stopped" } });
      case "/auth/delete": return reply({ ok: true });
      case "/settings/performance": return reply({}, 503);
      case "/settings/events": return reply(request.method() === "POST" ? { ok: true, settings: request.postDataJSON() } : { max_records: 500, overflow_policy: "drop_oldest" });
      case "/settings/compiler": return reply(request.method() === "POST" ? { ok: true, settings: request.postDataJSON() } : { resident: false, strategy: "on_demand" });
      case "/runner/agents": return reply({ enabled: true, total_agents: 0, online_agents: 0, agents: [] });
      case "/runner/jobs": return reply({ total_jobs: 1, jobs: [{ job_id: "job-a-full-confirmation-id", kind: "compile_check", state: "queued", owner_username: "student", target_agent_id: "agent-a", created_at: "2026-09-08T12:00:00Z" }] });
      case "/runner/jobs/cancel": return reply({ ok: true });
      default: errors.push(`Unexpected API ${request.method()} ${url.pathname}`); return reply({}, 404);
    }
  });
  return { browser, page, writes, errors, state, code: () => page.evaluate(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1"))) };
}

const dialog = page => page.getByRole("dialog", { name: "确认操作", exact: true });
const approve = page => dialog(page).getByRole("button", { name: /^确认/ });

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
