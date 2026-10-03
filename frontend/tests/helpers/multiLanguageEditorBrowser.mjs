import assert from "node:assert/strict";
import { baseUrl, engineUrl } from "./actionSafetyBrowser.mjs";

export { baseUrl, engineUrl };
export const confirmation = page => page.getByRole("dialog", { name: "确认操作", exact: true });
export const approve = page => confirmation(page).getByRole("button", { name: /^确认/ });

// A real Next page and real Monaco workers. Only session/unread reads are synthetic.
export async function setupEditor(browser, { role = "student", viewport = { width: 1440, height: 900 } } = {}) {
  const context = await browser.newContext({ viewport, acceptDownloads: true, serviceWorkers: "block" });
  const errors = [], engineRequests = [], assetRequests = [], networkRequests = [];
  context.on("request", request => {
    const url = new URL(request.url());
    networkRequests.push({ origin: url.origin, path: url.pathname, method: request.method() });
  });
  await context.addInitScript(() => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    // Simulate HTTP LAN contexts where secure-context-only randomUUID is unavailable.
    Object.defineProperty(Crypto.prototype, "randomUUID", { value: undefined, configurable: true });
  });
  const page = await context.newPage();
  page.setDefaultTimeout(10000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("websocket", () => errors.push("Unexpected WebSocket connection"));
  await context.route("**/*", route => {
    const request = route.request(), url = new URL(request.url());
    if (url.origin === new URL(baseUrl).origin && ["GET", "HEAD"].includes(request.method())) {
      if (url.pathname.startsWith("/monaco/")) assetRequests.push(url.pathname);
      return route.continue();
    }
    errors.push(`Unexpected network request: ${request.method()} ${url.origin}${url.pathname}`);
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, route => {
    const request = route.request(), url = new URL(request.url());
    const path = url.pathname, method = request.method();
    engineRequests.push({ path, method, bytes: request.postDataBuffer()?.length || 0 });
    const headers = {
      "access-control-allow-origin": new URL(baseUrl).origin,
      "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, OPTIONS",
      "access-control-allow-headers": "content-type",
      "cache-control": "no-store",
    };
    if (["/auth/me", "/events/unread-count"].includes(path)) {
      if (method === "OPTIONS") return route.fulfill({ status: 204, headers });
      if (method === "GET" && !url.search && !request.postData()) {
        return route.fulfill({ headers, json: path === "/auth/me"
          ? { authenticated: true, username: `editor-fixture-${role}`, role }
          : { unread: 0 } });
      }
    }
    errors.push(`Unexpected Engine request: ${method} ${path}`);
    return route.fulfill({ status: 403, headers, json: { message: "Editor fixtures forbid this operation" } });
  });
  try {
    await page.goto(`${baseUrl}/tasks/new`);
    await page.getByTestId("task-payload-add").click();
    await page.getByTestId("language-select").waitFor();
    await page.locator(".monaco-editor").waitFor();
    await page.waitForFunction(() => window.monaco?.editor.getModels().length === 1);
    await page.getByTestId("language-select").selectOption("typescript");
    await page.waitForFunction(() => window.monaco.editor.getModels()[0]?.getLanguageId() === "typescript");
    return {
      page, context, errors, engineRequests, assetRequests, networkRequests,
      source: () => page.evaluate(() => window.monaco.editor.getModels()[0].getValue()),
      async setSource(text) {
        await page.evaluate(value => {
          const editor = window.monaco.editor.getEditors()[0];
          editor.setValue(value);
          editor.setPosition(editor.getModel().getPositionAt(value.length));
          editor.focus();
        }, text);
        await page.waitForFunction(value => window.monaco.editor.getModels()[0]?.getValue() === value, text);
      },
      async selectLanguage(language) {
        await page.getByTestId("language-select").selectOption(language);
        await page.waitForFunction(value => window.monaco.editor.getModels()[0]?.getLanguageId() === value, language);
      },
      async assertNoLeaks() {
        assert.deepEqual(errors, []);
        assert.ok(engineRequests.some(request => request.path === "/auth/me"));
        assert.ok(engineRequests.every(request => ["GET", "OPTIONS"].includes(request.method)
          && ["/auth/me", "/events/unread-count"].includes(request.path) && request.bytes === 0));
        assert.ok(assetRequests.some(path => path.endsWith("/loader.js")), "Monaco must load from the local frontend");
      },
      close: () => context.close(),
    };
  } catch (error) {
    await context.close();
    throw error;
  }
}

export async function waitForMarkers(page, hasErrors) {
  await page.waitForFunction(expected => {
    const monaco = window.monaco, model = monaco.editor.getModels()[0];
    return Boolean(monaco.editor.getModelMarkers({ resource: model.uri })
      .some(marker => marker.severity === monaco.MarkerSeverity.Error)) === expected;
  }, hasErrors);
}
