import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
const id = "00000000-0000-4000-8000-000000000001";
const labId = "01-first-program";
const source = "// 继续这次提交\nint revision(void) { return 7; }";
const revised = `${source}\n// 已根据教师评语修改`;
const draft = "// 当前未保存的草稿\nint keep_me(void) { return 1; }";
const template = "// 模板不得覆盖草稿\nint template(void) { return 0; }";
const attempt = {
  id, username: "student", lab_id: labId, template_id: "xdp-pass", source, source_sha256: "a".repeat(64),
  run_success: false, stage: "compile", attach_expected: false, attach_verified: false, completed: false,
  feedback: ["Fix the guard"], teacher_feedback: { reviewer: "teacher", comment: "检查边界再运行", revision: 2, updated_at: "2026-09-08T12:00:00Z" },
  created_at: "2026-09-08T11:00:00Z",
};

async function setup() {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
  await context.addInitScript(({ draft }) => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    if (!sessionStorage.getItem("ebpf_code_v1")) sessionStorage.setItem("ebpf_code_v1", JSON.stringify(draft));
  }, { draft });
  const page = await context.newPage();
  const errors = [], runs = [], reads = [];
  const state = { status: 200, payload: attempt, deferred: null };
  page.on("pageerror", error => errors.push(error.message));
  await context.route(`${engineUrl}/**`, async route => {
    const request = route.request();
    const url = new URL(request.url());
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type" };
    const reply = (json, status = 200) => route.fulfill({ json, status, headers });
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    switch (url.pathname) {
      case "/auth/me": return reply({ authenticated: true, username: "student", role: "student" });
      case "/events/unread-count": return reply({ unread: 0 });
      case "/learning/attempts": return reply([attempt]);
      case "/learning/attempt":
        reads.push(url.search);
        if (state.deferred) return state.deferred(route, headers);
        return reply(state.payload, state.status);
      case "/learning/labs": return reply([{ lab: { id: labId, title: "第一个程序", template_id: "xdp-pass", doc_slug: "labs/01-first-program" },
        status: "in_progress", attempts: 1, latest_feedback: ["Fix the guard"] }]);
      case "/ebpf/templates": return reply([{ id: "xdp-pass", name: "Template", code: template, capability: "xdp" }]);
      case "/ebpf/attachments/details": return reply({ attachments: [] });
      case "/modules/c-headers/selected-metadata": return reply({ selected_headers: [] });
      case "/ebpf/check/backends": return reply({ local_available: true, agents: [] });
      case "/scripts": return reply([]);
      case "/ebpf/check": return reply({ ok: true, diagnostics: [], message: "checked", stdout: "", stderr: "" });
      case "/ebpf/complete": return reply({ ok: true, items: [], message: "" });
      case "/ebpf/run":
        runs.push(request.postDataJSON());
        return reply({ success: false, stage: "compile", message: "mocked compile failure", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "" });
      default: errors.push(`Unexpected API: ${request.method()} ${url.pathname}`); return reply({ ok: false }, 404);
    }
  });
  const code = () => page.evaluate(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1")));
  return { browser, page, errors, runs, reads, state, code };
}

test("resume previews feedback, preserves the draft, and restores source only after confirmation", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/learn`);
    const link = f.page.getByRole("link", { name: "从这次提交继续", exact: true });
    await link.waitFor();
    assert.equal(await link.getAttribute("href"), `/ebpf?lab=${labId}&attempt=${id}`);
    await link.click();
    await f.page.getByRole("heading", { name: "继续历史提交", exact: true }).waitFor();
    await f.page.getByText("检查边界再运行", { exact: true }).waitFor();
    await f.page.locator(".monaco-editor").waitFor();
    assert.match(await f.page.locator(".monaco-editor .view-lines").innerText(), /keep_me/);
    if (process.env.CYANREX_UI_SCREENSHOT) {
      await f.page.screenshot({ path: `${process.env.CYANREX_UI_SCREENSHOT}.before.png`, fullPage: false });
      await f.page.setViewportSize({ width: 390, height: 844 });
      await f.page.locator(".learning-resume").scrollIntoViewIfNeeded();
      await f.page.waitForFunction(() => document.documentElement.scrollWidth <= innerWidth, undefined, { timeout: 3000 }).catch(async () => {
        console.log(await f.page.evaluate(() => [...document.querySelectorAll("body *")].filter(el => el.getBoundingClientRect().right > innerWidth + 1)
          .slice(0, 12).map(el => ({ tag: el.tagName, class: el.className, width: el.getBoundingClientRect().width }))));
      });
      await f.page.screenshot({ path: `${process.env.CYANREX_UI_SCREENSHOT}.mobile.png`, fullPage: false });
      assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      await f.page.setViewportSize({ width: 1280, height: 1000 });
    }
    assert.equal(await f.code(), draft);
    assert.equal(await f.page.getByRole("button", { name: "编译并运行", exact: true }).isDisabled(), true);
    assert.equal(f.runs.length, 0);
    await f.page.getByRole("button", { name: "替换当前草稿并继续", exact: true }).click();
    await f.page.waitForFunction(source => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === source, source);
    assert.match(await f.page.locator(".monaco-editor .view-lines").innerText(), /revision/);
    assert.equal(f.runs.length, 0, "confirmation must not run code");
    assert.equal(await f.page.getByRole("button", { name: "编译并运行", exact: true }).isEnabled(), true);
    if (process.env.CYANREX_UI_SCREENSHOT) await f.page.screenshot({ path: process.env.CYANREX_UI_SCREENSHOT, fullPage: false });
    await f.page.locator(".monaco-editor").click({ position: { x: 160, y: 20 } });
    await f.page.keyboard.press("ControlOrMeta+A");
    await f.page.keyboard.insertText(revised);
    await f.page.waitForFunction(revised => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === revised, revised);
    await f.page.getByRole("button", { name: "编译并运行", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认编译并运行", exact: true }).click();
    await f.page.getByText("mocked compile failure", { exact: false }).first().waitFor();
    assert.equal(f.runs.length, 1);
    assert.equal(f.runs[0].code, revised);
    assert.equal(f.runs[0].lab_id, labId);
    assert.equal(f.runs[0].template_id, "xdp-pass");
    await f.page.reload();
    await f.page.getByRole("button", { name: "替换当前草稿并继续", exact: true }).waitFor();
    assert.equal(await f.code(), revised, "reload must not replace the revised code with the template");
    await f.page.getByRole("button", { name: "保留当前草稿", exact: true }).click();
    assert.equal(await f.code(), revised);
    assert.equal(f.runs.length, 1);
    await f.page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("client-side target changes discard late source and require a fresh confirmation on revisit", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    const first = `/ebpf?lab=${labId}&attempt=${id}`;
    await f.page.goto(`${baseUrl}${first}`);
    await f.page.getByRole("button", { name: "替换当前草稿并继续", exact: true }).click();
    await f.page.waitForFunction(source => JSON.parse(sessionStorage.getItem("ebpf_code_v1")) === source, source);
    const otherId = "00000000-0000-4000-8000-000000000002";
    let release, started;
    const waiting = new Promise(resolve => { started = resolve; });
    f.state.deferred = async (route, headers) => {
      await new Promise(resolve => { release = resolve; started(); });
      await route.fulfill({ json: { ...attempt, id: otherId, source: "stale response must not appear" }, headers }).catch(() => {});
    };
    // Exercise Next's same-page router transition without destroying the editor/controller instance.
    await f.page.evaluate(url => window.next.router.push(url), `/ebpf?lab=${labId}&attempt=${otherId}`);
    await waiting;
    assert.equal(await f.page.getByRole("button", { name: "编译并运行", exact: true }).isDisabled(), true);
    f.state.deferred = null;
    await f.page.evaluate(url => window.next.router.push(url), first);
    await f.page.getByRole("button", { name: "替换当前草稿并继续", exact: true }).waitFor();
    release();
    assert.equal(await f.page.getByRole("button", { name: "编译并运行", exact: true }).isDisabled(), true,
      "the earlier confirmation must not carry over to a revisited target");
    assert.equal(await f.code(), source);
    assert.equal(await f.page.getByText("stale response must not appear", { exact: true }).count(), 0);
    assert.equal(f.runs.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("failed, malformed and cancelled resume requests never replace the editor draft", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    f.state.status = 404;
    await f.page.goto(`${baseUrl}/ebpf?lab=${labId}&attempt=${id}`);
    await f.page.getByRole("alert").filter({ hasText: "历史提交加载失败" }).waitFor();
    assert.equal(await f.code(), draft);
    f.state.status = 200;
    await f.page.getByRole("button", { name: "重试加载", exact: true }).click();
    await f.page.getByRole("button", { name: "替换当前草稿并继续", exact: true }).waitFor();
    await f.page.getByRole("button", { name: "保留当前草稿", exact: true }).click();
    assert.equal(await f.code(), draft);
    const before = f.reads.length;
    await f.page.goto(`${baseUrl}/ebpf?lab=${labId}&attempt=invalid`);
    await f.page.getByRole("alert").filter({ hasText: "历史提交链接无效" }).waitFor();
    assert.equal(f.reads.length, before);
    assert.equal(await f.code(), draft);
    let release, started;
    const waiting = new Promise(resolve => { started = resolve; });
    f.state.deferred = async (route, headers) => {
      await new Promise(resolve => { release = resolve; started(); });
      await route.fulfill({ json: attempt, headers }).catch(() => {});
    };
    await f.page.goto(`${baseUrl}/ebpf?lab=${labId}&attempt=${id}`);
    await f.page.getByRole("status").filter({ hasText: "正在读取历史提交" }).waitFor();
    await waiting;
    await f.page.getByRole("button", { name: "保留当前草稿", exact: true }).click();
    release();
    await f.page.getByRole("status").filter({ hasText: "已保留当前草稿" }).waitFor();
    assert.equal(await f.code(), draft);
    assert.equal(f.runs.length, 0);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
