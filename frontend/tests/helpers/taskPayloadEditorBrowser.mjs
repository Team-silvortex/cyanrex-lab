import assert from "node:assert/strict";
import { baseUrl, engineUrl } from "./actionSafetyBrowser.mjs";

export { baseUrl, engineUrl };
export const confirmation = page => page.getByRole("dialog", { name: "确认操作", exact: true });
export const approve = page => confirmation(page).getByRole("button", { name: /^确认/ });
export const textItem = (id, overrides = {}) => ({
  id, revision: 1, kind: "text", filename: "untitled.txt", language: "plaintext", text: "", ...overrides,
});
export const taskDraft = (payload = [], title = "") => ({ format: "cyanrex.task-draft", version: 1, title, payload });

// Real Next routes and Monaco models; only identity and unread-count reads are mocked.
export async function setupTask(browser, { path = "/tasks/new", viewport = { width: 1440, height: 900 } } = {}) {
  const context = await browser.newContext({ viewport, acceptDownloads: true, serviceWorkers: "block" });
  const errors = [], engineRequests = [];
  await context.addInitScript(() => localStorage.setItem("cyanrex_locale", "zh-CN"));
  const page = await context.newPage();
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
    const request = route.request(), url = new URL(request.url()), method = request.method();
    engineRequests.push({ path: url.pathname, method, bytes: request.postDataBuffer()?.length || 0 });
    const headers = {
      "access-control-allow-origin": new URL(baseUrl).origin,
      "access-control-allow-credentials": "true", "access-control-allow-methods": "GET, OPTIONS",
      "access-control-allow-headers": "content-type", "cache-control": "no-store",
    };
    if (["/auth/me", "/events/unread-count"].includes(url.pathname)) {
      if (method === "OPTIONS") return route.fulfill({ status: 204, headers });
      if (method === "GET" && !url.search && !request.postData()) {
        return route.fulfill({ headers, json: url.pathname === "/auth/me"
          ? { authenticated: true, username: "payload-fixture-student", role: "student" } : { unread: 0 } });
      }
    }
    errors.push(`Unexpected Engine request: ${method} ${url.pathname}`);
    return route.fulfill({ status: 403, headers, json: { message: "Task payload fixtures forbid network writes" } });
  });
  try {
    await page.goto(`${baseUrl}${path}`);
    await page.getByTestId("task-title").waitFor();
    const items = page.getByTestId("task-payload-list").locator("button[data-payload-id]");
    const fixture = {
      page, errors, engineRequests, items,
      ids: () => items.evaluateAll(buttons => buttons.map(button => button.dataset.payloadId)),
      item: id => page.getByTestId("task-payload-list").locator(`button[data-payload-id=${JSON.stringify(id)}]`),
      source: () => page.evaluate(() => window.monaco.editor.getModels()[0]?.getValue()),
      async add() {
        const before = await fixture.ids();
        await page.getByTestId("task-payload-add").click();
        await page.waitForFunction(count => document.querySelectorAll('[data-testid="task-payload-list"] button[data-payload-id]').length === count, before.length + 1);
        await page.locator(".monaco-editor").waitFor();
        await page.waitForFunction(() => window.monaco?.editor.getModels().length === 1);
        const id = (await fixture.ids()).find(value => !before.includes(value));
        assert.ok(id, "adding text creates a fresh payload identity");
        return id;
      },
      async setSource(text) {
        await page.evaluate(value => {
          const editor = window.monaco.editor.getEditors()[0];
          editor.setValue(value);
          editor.setPosition(editor.getModel().getPositionAt(value.length));
        }, text);
        await page.waitForFunction(value => window.monaco.editor.getModels()[0]?.getValue() === value, text);
      },
      async language(language) {
        await page.getByTestId("language-select").selectOption(language);
        await page.waitForFunction(value => window.monaco.editor.getModels()[0]?.getLanguageId() === value, language);
      },
      async current({ filename, language, text }) {
        await page.waitForFunction(expected => {
          const model = window.monaco?.editor.getModels()[0];
          return model?.getValue() === expected.text && model.getLanguageId() === expected.language
            && document.querySelector('[data-testid="editor-filename"]')?.value === expected.filename;
        }, { filename, language, text });
        assert.equal(await page.evaluate(() => window.monaco.editor.getModels().length), 1);
      },
      async export() {
        const pending = page.waitForEvent("download");
        await page.getByTestId("task-draft-export").click();
        const download = await pending, chunks = [];
        for await (const chunk of await download.createReadStream()) chunks.push(chunk);
        assert.match(download.suggestedFilename(), /\.json$/i);
        return JSON.parse(Buffer.concat(chunks).toString("utf8"));
      },
      async chooseImport(value) {
        const buffer = Buffer.isBuffer(value) ? value : Buffer.from(JSON.stringify(value));
        await page.getByTestId("task-draft-import").setInputFiles({ name: "task-fixture.json", mimeType: "application/json", buffer });
      },
      async import(value) {
        await fixture.chooseImport(value);
        await confirmation(page).waitFor();
        await approve(page).click();
        await confirmation(page).waitFor({ state: "detached" });
      },
      async rejectImport(value) {
        await fixture.chooseImport(value);
        await confirmation(page).waitFor();
        await approve(page).click();
        await confirmation(page).getByRole("alert").waitFor();
        await confirmation(page).getByRole("button", { name: "关闭", exact: true }).click();
        await confirmation(page).waitFor({ state: "detached" });
      },
      async assertLocalOnly() {
        assert.deepEqual(errors, []);
        assert.ok(engineRequests.some(request => request.path === "/auth/me"));
        assert.ok(engineRequests.every(request => ["/auth/me", "/events/unread-count"].includes(request.path)
          && ["GET", "OPTIONS"].includes(request.method) && request.bytes === 0));
      },
      close: () => context.close(),
    };
    return fixture;
  } catch (error) { await context.close(); throw error; }
}

export async function waitForErrors(page, expected) {
  await page.waitForFunction(value => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0];
    return monaco.editor.getModelMarkers({ resource: model.uri })
      .some(marker => marker.severity === monaco.MarkerSeverity.Error) === value;
  }, expected);
}
