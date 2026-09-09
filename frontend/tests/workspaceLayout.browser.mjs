import assert from "node:assert/strict";
import test from "node:test";

// Real layout/Monaco checks; all Engine traffic is synthetic and cannot load a kernel program.
const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
const draft = "// Layout test draft\nint keep_layout(void) { return 1; }";
const lab = { id: "01-first-program", title: "第一个程序", position: 1, template_id: "xdp-pass", doc_slug: "labs/01-first-program",
  summary: "从编辑源码到查看结果，在同一个工作区完成实验。" };
const progress = { lab, status: "in_progress", attempts: 1, latest_feedback: [], latest_stage: "compile" };
const username = "student-with-a-long-display-name-for-layout-testing";

async function setup(role = "student", viewport = { width: 1440, height: 900 }) {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport });
  await context.addInitScript(draft => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    sessionStorage.setItem("ebpf_code_v1", JSON.stringify(draft));
  }, draft);
  const page = await context.newPage();
  const errors = [], writes = [], saves = [], scripts = [];
  page.on("pageerror", error => errors.push(error.message));
  await context.route(`${engineUrl}/**`, async route => {
    const request = route.request(), url = new URL(request.url());
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type" };
    const reply = (json, status = 200) => route.fulfill({ json, status, headers });
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    switch (url.pathname) {
      case "/auth/me": return reply({ authenticated: true, username: role, role });
      case "/events/unread-count": return reply({ unread: 3 });
      case "/learning/labs": return reply([progress]);
      case "/learning/attempts": return reply([]);
      case "/learning/teacher/overview": return reply({ active_students: 1, total_labs: 5, students: [
        { username, completed_labs: 1, total_labs: 5, total_attempts: 1, last_activity_at: "2026-09-08T12:00:00Z", labs: [progress] },
      ] });
      case "/learning/teacher/attempts": return reply({ username, attempts: [{
        id: "00000000-0000-4000-8000-000000000001", username, lab_id: lab.id, template_id: "xdp-pass", source: draft,
        source_sha256: "a".repeat(64), run_success: false, stage: "compile", attach_expected: false, attach_verified: false,
        completed: false, feedback: [], teacher_feedback: null, created_at: "2026-09-08T12:00:00Z",
      }] });
      case "/ebpf/templates": return reply([{ id: "xdp-pass", name: "Template", code: draft, capability: "xdp" }]);
      case "/ebpf/attachments/details": return reply({ attachments: [] });
      case "/modules/c-headers/selected-metadata": return reply({ selected_headers: [] });
      case "/ebpf/check/backends": return reply({ local_available: true, agents: [] });
      case "/scripts": return reply(scripts);
      case "/scripts/save": {
        const body = request.postDataJSON();
        saves.push(body);
        scripts.push({ ...body, id: "layout-script", username: role, updated_at: "2026-09-08T12:00:00Z" });
        return reply({ ok: true, message: "saved" });
      }
      case "/ebpf/check": return reply({ ok: true, diagnostics: [], message: "checked", stdout: "", stderr: "" });
      case "/ebpf/complete": return reply({ ok: true, items: [], message: "" });
      case "/ebpf/run":
        writes.push(request.postDataJSON());
        return reply({ success: false, stage: "compile", message: "Layout test compile result", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "" });
      default: errors.push(`Unexpected API: ${request.method()} ${url.pathname}`); return reply({}, 404);
    }
  });
  return { browser, page, errors, writes, saves };
}

async function noOverflow(page) {
  await page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth, undefined, { timeout: 3000 });
}

async function screenshot(page, name) {
  if (process.env.CYANREX_UI_SCREENSHOT) await page.screenshot({ path: `${process.env.CYANREX_UI_SCREENSHOT}.${name}.png` });
}

test("desktop workspace prioritizes source, keeps run actions nearby, and preserves the editor when inspecting resources", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    const editor = f.page.locator(".monaco-editor");
    await editor.waitFor();
    const box = await editor.boundingBox();
    assert.ok(box.y < 330, `source should start in the first screen, not at ${box.y}px`);
    const settings = f.page.locator("#runtime-settings");
    await settings.waitFor();
    assert.ok((await settings.boundingBox()).x >= box.x + box.width, "runtime settings should sit beside the editor on desktop");
    await f.page.getByLabel("脚本标题", { exact: true }).fill("Layout draft");
    await f.page.getByLabel("运行后端", { exact: true }).selectOption("aya");
    await editor.click({ position: { x: 160, y: 20 } });
    await f.page.keyboard.press("ControlOrMeta+A");
    await f.page.keyboard.insertText(`${draft}\n// Still mounted`);
    const element = await editor.elementHandle();
    const metadata = f.page.locator("details").filter({ has: f.page.locator("summary", { hasText: "内联元数据" }) }).first();
    await metadata.locator("summary").click();
    await metadata.locator("summary").click();
    assert.equal(await element.evaluate(el => el.isConnected), true, "disclosures must not remount Monaco");
    assert.equal(f.writes.length, 0, "layout and settings changes must not run code");
    await f.page.getByRole("button", { name: "编译并运行", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认编译并运行", exact: true }).click();
    await f.page.getByRole("alert").filter({ hasText: "Layout test compile result" }).waitFor();
    assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0].code, `${draft}\n// Still mounted`);
    assert.equal(f.writes[0].runtime_backend, "aya");
    await f.page.locator("#ebpf-output").scrollIntoViewIfNeeded();
    const runBox = await f.page.getByRole("button", { name: "编译并运行", exact: true }).boundingBox();
    assert.ok(runBox.y >= 0 && runBox.y + runBox.height <= 900, "run action remains visible while reading output");
    await noOverflow(f.page);
    await f.page.evaluate(() => scrollTo(0, 0));
    await screenshot(f.page, "editor-desktop");
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("compact navigation supports keyboard dismissal, closes on navigation, and leaves learning work above the fold", { timeout: 45000 }, async () => {
  const f = await setup("student", { width: 390, height: 844 });
  try {
    await f.page.goto(`${baseUrl}/learn`);
    const menu = f.page.getByRole("button", { name: "导航菜单", exact: true });
    await menu.waitFor();
    assert.equal(await menu.getAttribute("aria-expanded"), "false");
    assert.equal(await f.page.getByRole("navigation", { name: "主导航", exact: true }).isVisible(), false);
    const heading = f.page.getByRole("heading", { name: "实验进度", exact: true });
    await heading.waitFor();
    assert.ok((await heading.boundingBox()).y < 400, "progress should precede documentation cards");
    await menu.click();
    assert.equal(await menu.getAttribute("aria-expanded"), "true");
    assert.equal(await f.page.getByRole("link", { name: "教学管理", exact: true }).count(), 0);
    await f.page.keyboard.press("Escape");
    assert.equal(await menu.getAttribute("aria-expanded"), "false");
    assert.equal(await menu.evaluate(el => el === document.activeElement), true);
    for (const width of [320, 390, 768, 1024]) {
      await f.page.setViewportSize({ width, height: 844 });
      await noOverflow(f.page);
    }
    await f.page.setViewportSize({ width: 390, height: 844 });
    await screenshot(f.page, "learn-mobile");
    await f.page.getByRole("link", { name: "我的实验记录", exact: true }).click();
    const history = await f.page.getByRole("heading", { name: "我的实验记录", exact: true }).boundingBox();
    assert.ok(history.y >= 65 && history.y < 300, "anchor target must not hide under the compact header");
    await menu.click();
    await f.page.getByRole("navigation", { name: "主导航", exact: true }).getByRole("link", { name: "eBPF 运行器", exact: true }).click();
    await f.page.locator(".monaco-editor").waitFor();
    const sourceEditor = await f.page.locator(".monaco-editor").elementHandle();
    assert.equal(await menu.getAttribute("aria-expanded"), "false");
    assert.ok((await f.page.locator(".monaco-editor").boundingBox()).y < 560);
    await f.page.getByRole("link", { name: "运行配置", exact: true }).click();
    assert.ok((await f.page.locator("#runtime-settings").boundingBox()).y >= 65);
    await f.page.getByRole("link", { name: "返回源码", exact: true }).click();
    assert.equal(await sourceEditor.evaluate(el => el.isConnected), true, "section links must preserve the mounted editor");
    for (const width of [320, 390, 768, 1024]) {
      await f.page.setViewportSize({ width, height: 844 });
      await noOverflow(f.page);
    }
    await f.page.setViewportSize({ width: 390, height: 844 });
    await f.page.evaluate(() => scrollTo(0, 0));
    await screenshot(f.page, "editor-mobile");
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("classroom roster fits narrow screens and selecting a student brings the review into view", { timeout: 45000 }, async () => {
  const f = await setup("teacher", { width: 390, height: 844 });
  try {
    await f.page.goto(`${baseUrl}/teaching`);
    const action = f.page.getByRole("button", { name: "查看尝试", exact: true });
    await action.waitFor();
    await noOverflow(f.page);
    assert.ok((await action.boundingBox()).x >= 0);
    assert.ok((await action.boundingBox()).x + (await action.boundingBox()).width <= 390);
    await screenshot(f.page, "classroom-mobile");
    await action.click();
    await f.page.getByLabel("编写评语", { exact: true }).waitFor();
    const title = f.page.locator("#attempt-review-title");
    assert.equal(await title.evaluate(el => document.activeElement === el), true);
    const box = await title.boundingBox();
    assert.ok(box.y >= 65 && box.y < 300, `review heading must be visible below the mobile header (${box.y})`);
    await f.page.getByLabel("编写评语", { exact: true }).fill("保留这份评语草稿");
    await noOverflow(f.page);
    await f.page.getByRole("link", { name: "返回学生列表", exact: true }).click();
    assert.ok((await f.page.locator("#student-progress").boundingBox()).y >= 65);
    await f.page.setViewportSize({ width: 1440, height: 900 });
    assert.equal(await f.page.getByLabel("编写评语", { exact: true }).inputValue(), "保留这份评语草稿");
    await f.page.evaluate(() => scrollTo(0, 0));
    await screenshot(f.page, "classroom-desktop");
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("all four interface languages fit a compact editor and navigation", { timeout: 45000 }, async () => {
  const f = await setup("admin", { width: 320, height: 844 });
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    for (const locale of ["en", "es", "ja", "zh-CN"]) {
      await f.page.locator(".menu-toggle").click();
      await f.page.locator(".sidebar-footer select").selectOption(locale);
      await noOverflow(f.page);
      assert.equal(await f.page.locator('.nav-link[aria-current="page"]').count(), 1);
      await f.page.keyboard.press("Escape");
      assert.equal(await f.page.locator(".menu-toggle").getAttribute("aria-expanded"), "false");
      await noOverflow(f.page);
      const box = await f.page.locator(".editor-actions .button-primary").boundingBox();
      assert.ok(box.x >= 0 && box.x + box.width <= 320);
    }
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("import, save and reload remain reachable without running the program", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/ebpf`);
    await f.page.locator(".monaco-editor").waitFor();
    const imported = "// Imported draft\nint imported_source(void) { return 8; }";
    const chooser = f.page.waitForEvent("filechooser");
    await f.page.getByRole("button", { name: "导入文件", exact: true }).click();
    await (await chooser).setFiles({ name: "layout.c", mimeType: "text/plain", buffer: Buffer.from(imported) });
    await f.page.getByRole("dialog").getByRole("button", { name: "确认导入文件", exact: true }).click();
    await f.page.waitForFunction(value => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === value, imported);
    await f.page.getByLabel("脚本标题", { exact: true }).fill("Saved layout draft");
    await f.page.getByRole("button", { name: "保存脚本", exact: true }).click();
    const saved = f.page.locator("details.workspace-disclosure").filter({ has: f.page.locator("summary", { hasText: "已保存脚本" }) });
    await saved.locator("summary").click();
    await saved.getByText("Saved layout draft", { exact: true }).waitFor();
    assert.deepEqual(f.saves, [{ title: "Saved layout draft", script: imported }]);
    await f.page.locator(".monaco-editor").click({ position: { x: 160, y: 20 } });
    await f.page.keyboard.press("ControlOrMeta+A");
    await f.page.keyboard.insertText(draft);
    await saved.getByRole("button", { name: "加载", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认加载", exact: true }).click();
    await f.page.waitForFunction(value => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === value, imported);
    assert.equal(f.writes.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
