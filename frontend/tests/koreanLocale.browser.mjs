import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { registerHooks } from "node:module";
import { baseUrl, engineUrl, setupTask, taskDraft, textItem } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
async function korean() {
  const root = new URL("../src/i18n/", import.meta.url).href;
  const resolver = registerHooks({ resolve(s, c, next) {
    return c.parentURL?.startsWith(root) && s.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(s)
      ? next(new URL(`${s}.ts`, c.parentURL).href, c) : next(s, c);
  } });
  try {
    const { translations } = await import("../src/i18n/translations.ts");
    assert.ok(translations.ko);
    return (key, variables = {}) => {
      const value = key.split(".").reduce((object, part) => object?.[part], translations.ko);
      assert.equal(typeof value, "string", key);
      return value.replace(/\{(\w+)\}/g, (_, name) => String(variables[name]));
    };
  } finally { resolver.deregister(); }
}
async function run(body, options) {
  const f = await setupTask(browser, options);
  let failure;
  try { await body(f); } catch (error) { failure = error; }
  finally {
    try { await f.assertLocalOnly(); }
    catch (error) { failure = failure ? new AggregateError([failure, error], "Behavior and isolation failed") : error; }
    finally { await f.close(); }
  }
  if (failure) throw failure;
}

test("Korean selection updates the real editor without changing source identity revisions or undo", { timeout: 60000 }, () => run(async f => {
  const source = "// 원본 한글 코드\nfn";
  await f.import(taskDraft([textItem("korean_item", { filename: "설명.rs", language: "rust", text: source })], "한글 작업"));
  await f.current({ filename: "설명.rs", language: "rust", text: source });
  await f.page.evaluate(() => {
    const editor = window.monaco.editor.getEditors()[0];
    editor.pushUndoStop(); editor.executeEdits("korean-test", [{ range: new window.monaco.Range(1, 1, 1, 1), text: "// undo\n" }]);
    editor.pushUndoStop();
  });
  const before = await f.export(), model = await f.page.evaluateHandle(() => window.monaco.editor.getModels()[0]);
  try {
    await f.page.locator(".sidebar-footer select").selectOption("ko");
    const t = await korean();
    await f.page.waitForFunction(() => document.documentElement.lang === "ko");
    assert.equal(await f.page.locator(".sidebar-footer select option[value=ko]").textContent(), "한국어");
    assert.equal(await f.page.getByTestId("language-select").locator('option[value="plaintext"]').textContent(), t("editorIntelligence.plainText"));
    assert.equal(await f.page.locator('.monaco-editor [role="textbox"]').getAttribute("aria-label"), t("editorIntelligence.ariaLabel"));
    assert.equal(await model.evaluate(value => !value.isDisposed() && value === window.monaco.editor.getModels()[0]), true);
    assert.deepEqual(await f.export(), before);
    await f.page.evaluate(() => {
      const editor = window.monaco.editor.getEditors()[0];
      editor.setPosition(editor.getModel().getPositionAt(editor.getModel().getValueLength()));
    });
    await f.page.getByTestId("editor-completion").click();
    await f.page.locator(".suggest-widget .monaco-list-row").filter({ hasText: t("editorIntelligence.snippets.rust.fn") }).waitFor();
    await f.page.keyboard.press("Escape");
    await f.page.evaluate(() => window.monaco.editor.getEditors()[0].trigger("korean-test", "undo", null));
    await f.current({ filename: "설명.rs", language: "rust", text: source });
    assert.deepEqual(await f.export(), { ...before, payload: [{ ...before.payload[0], revision: before.payload[0].revision + 1, text: source }] });
  } finally { await model.dispose(); }
}));

test("Korean destructive confirmation cancels safely and controls fit narrow screens", { timeout: 60000 }, () => run(async f => {
  await f.page.locator(".menu-toggle").click();
  await f.page.locator(".sidebar-footer select").selectOption("ko");
  await f.page.keyboard.press("Escape");
  const t = await korean();
  await f.page.waitForFunction(() => document.documentElement.lang === "ko");
  const id = await f.add(); await f.setSource("보존할 한글 내용");
  const before = await f.export();
  for (const width of [390, 320]) {
    await f.page.setViewportSize({ width, height: 900 });
    await f.page.getByTestId("task-payload-remove").click();
    const dialog = f.page.getByRole("dialog", { name: t("safety.title"), exact: true });
    await dialog.waitFor();
    await dialog.getByText(before.payload[0].filename, { exact: true }).waitFor();
    await dialog.getByText(t("taskDraft.removeConfirm"), { exact: true }).waitFor();
    assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    if (process.env.CYANREX_UI_SCREENSHOTS) await f.page.screenshot({
      path: `${process.env.CYANREX_UI_SCREENSHOTS}/korean-confirmation-${width}.png`, fullPage: true,
    });
    await dialog.getByRole("button", { name: t("safety.cancel"), exact: true }).click();
    await dialog.waitFor({ state: "detached" });
    assert.deepEqual(await f.export(), before);
    assert.equal(await f.item(id).getAttribute("aria-pressed"), "true");
    for (const control of ["task-title", "task-text-import-open", "task-draft-export", "editor-filename"]) {
      const box = await f.page.getByTestId(control).boundingBox();
      assert.ok(box && box.x >= 0 && box.x + box.width <= width, control);
    }
    assert.doesNotMatch(await f.page.locator("main").innerText(), /\b(?:taskDraft|editorIntelligence|layout)\.[A-Za-z]/);
  }
}, { viewport: { width: 390, height: 900 } }));

test("Korean preference persists after reload and missing course translations visibly fall back to English", { timeout: 60000 }, async () => {
  // A fresh context has no fixture script that resets preferences on every navigation.
  const context = await browser.newContext({ viewport: { width: 390, height: 900 }, serviceWorkers: "block" });
  const page = await context.newPage(), errors = [], requests = [], engineRequests = [];
  page.setDefaultTimeout(10000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("websocket", () => errors.push("Unexpected WebSocket connection"));
  await context.route("**/*", route => {
    const request = route.request(), url = new URL(request.url());
    if (url.origin === new URL(baseUrl).origin && ["GET", "HEAD"].includes(request.method())) return route.continue();
    errors.push(`Unexpected request: ${request.method()} ${url.origin}${url.pathname}`);
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, route => {
    const request = route.request(), url = new URL(request.url());
    engineRequests.push({ path: url.pathname, method: request.method(), bytes: request.postDataBuffer()?.length || 0 });
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, OPTIONS", "access-control-allow-headers": "content-type", "cache-control": "no-store" };
    if (["/auth/me", "/events/unread-count"].includes(url.pathname)) {
      if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
      if (request.method() === "GET" && !url.search && !request.postData()) return route.fulfill({ headers,
        json: url.pathname === "/auth/me" ? { authenticated: true, username: "korean-fixture-student", role: "student" } : { unread: 0 } });
    }
    errors.push(`Unexpected Engine request: ${request.method()} ${url.pathname}`);
    return route.abort();
  });
  page.on("request", request => {
    const url = new URL(request.url());
    if (url.pathname.startsWith("/course/")) requests.push(url.pathname);
  });
  let failure;
  try {
    await page.goto(`${baseUrl}/login`);
    assert.equal(await page.locator("select").inputValue(), "en");
    await page.locator("select").selectOption("ko");
    const t = await korean();
    await page.getByRole("heading", { name: t("auth.login"), exact: true }).waitFor();
    await page.reload();
    await page.waitForFunction(() => document.documentElement.lang === "ko");
    assert.equal(await page.locator("select").inputValue(), "ko");
    await page.goto(`${baseUrl}/learn/editor`);
    await page.getByText(t("learn.documentFallback", { language: "English" }), { exact: true }).waitFor();
    await page.getByRole("heading", { name: "Task payload editing", exact: true }).waitFor();
    assert.equal(await page.locator(".markdown-document").getAttribute("lang"), "en");
    assert.equal(await page.evaluate(() => document.documentElement.lang), "ko");
    const attempt = requests.indexOf("/course/ko/editor.md");
    assert.ok(attempt >= 0); assert.ok(requests.indexOf("/course/en/editor.md", attempt + 1) > attempt);
    assert.equal(requests.some(path => path.startsWith("/course/zh-CN/")), false);
    await page.goto(`${baseUrl}/login`);
    await page.getByRole("heading", { name: t("auth.login"), exact: true }).waitFor();
    assert.equal(await page.locator("select").inputValue(), "ko");
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  } catch (error) { failure = error; }
  finally {
    try {
      assert.deepEqual(errors, []);
      assert.ok(engineRequests.some(request => request.path === "/auth/me"));
      assert.ok(engineRequests.every(request => ["/auth/me", "/events/unread-count"].includes(request.path)
        && ["GET", "OPTIONS"].includes(request.method) && request.bytes === 0));
    } catch (error) { failure = failure ? new AggregateError([failure, error], "Behavior and isolation failed") : error; }
    finally { await context.close(); }
  }
  if (failure) throw failure;
});

test("an obsolete Korean course body cannot replace a newer language selection", { timeout: 60000 }, () => run(async f => {
  // Deliberately delay body completion even after abort to exercise the page's ownership check.
  await f.page.context().addInitScript(() => {
    const original = window.fetch;
    let reads = 0;
    window.fetch = async (input, options) => {
      if (input !== "/course/ko/editor.md") return original(input, options);
      reads += 1;
      if (reads > 1) return new Response("# 최신 한국어 본문");
      return { ok: true, text: async () => {
        const body = await new Promise(resolve => { window.releaseKoreanBody = () => resolve("# 지연된 한국어 본문"); });
        window.koreanBodyFinished = true;
        return body;
      } };
    };
  });
  await f.page.goto(`${baseUrl}/learn/editor`);
  await f.page.locator(".markdown-document").waitFor();
  await f.page.locator(".sidebar-footer select").selectOption("ko");
  await f.page.waitForFunction(() => typeof window.releaseKoreanBody === "function");
  await f.page.locator(".sidebar-footer select").selectOption("en");
  await f.page.getByRole("heading", { name: "Task payload editing", exact: true }).waitFor();
  await f.page.evaluate(async () => {
    window.releaseKoreanBody();
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  assert.equal(await f.page.evaluate(() => window.koreanBodyFinished), true);
  assert.equal(await f.page.locator(".markdown-document").getAttribute("lang"), "en");
  assert.equal(await f.page.getByRole("heading", { name: "Task payload editing", exact: true }).count(), 1);
  assert.equal(await f.page.getByText("지연된 한국어 본문", { exact: true }).count(), 0);
  await f.page.locator(".sidebar-footer select").selectOption("ko");
  await f.page.getByRole("heading", { name: "최신 한국어 본문", exact: true }).waitFor();
  assert.equal(await f.page.locator(".markdown-document").getAttribute("lang"), "ko");
}));
