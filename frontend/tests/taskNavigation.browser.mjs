import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { baseUrl, engineUrl, setupTask } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

test("removing a task-page fragment through the current sidebar link preserves the entire local draft", { timeout: 60000 }, async () => {
  const f = await setupTask(browser, { path: "/tasks/new#main-content" });
  const dialogs = [];
  f.page.on("dialog", async dialog => { dialogs.push(dialog.type()); await dialog.dismiss(); });
  try {
    await f.page.getByTestId("task-title").fill("Keep this local task on the same page");
    await f.add();
    await f.page.getByTestId("editor-filename").fill("private-notes.md");
    await f.setSource("Private payload must survive a same-page link.");
    const before = await f.export();
    const editor = await f.page.locator(".monaco-editor").elementHandle();
    const authReads = () => f.engineRequests.filter(request => request.path === "/auth/me").length;
    const initialAuthReads = authReads();
    await f.page.locator('nav a[href="/tasks/new"]').click();
    await f.page.waitForURL(`${baseUrl}/tasks/new`);
    await f.page.getByTestId("task-title").waitFor();
    assert.deepEqual(await f.export(), before, "fragment navigation must not replace the task owner");
    assert.equal(await editor.evaluate(element => element.isConnected), true, "the displayed editor must stay mounted");
    assert.equal(authReads(), initialAuthReads, "an anchor is not a new authorization route");
    assert.deepEqual(dialogs, [], "same-page anchors should not ask to discard the draft");
    await f.assertLocalOnly();
  } finally { await f.close(); }
});

test("changing the task query still rechecks identity and rejects an expired session", { timeout: 30000 }, async () => {
  const f = await setupTask(browser);
  let authChecks = 0;
  try {
    await f.page.context().route(`${engineUrl}/auth/me`, route => {
      authChecks++;
      return route.fulfill({ headers: {
        "access-control-allow-origin": new URL(baseUrl).origin,
        "access-control-allow-credentials": "true", "cache-control": "no-store",
      }, json: { authenticated: false } });
    });
    await f.page.evaluate(() => window.next.router.push("/tasks/new?view=another"));
    await f.page.waitForURL(url => url.pathname === "/login");
    assert.equal(authChecks, 1, "non-fragment route changes must still recheck the session");
    assert.equal(new URL(f.page.url()).searchParams.get("next"), "/tasks/new?view=another");
    assert.equal(await f.page.getByTestId("task-title").count(), 0);
    await f.assertLocalOnly();
  } finally { await f.close(); }
});

test("same-page fragment navigation does not cancel a pending confirmed logout", { timeout: 30000 }, async () => {
  const f = await setupTask(browser, { path: "/tasks/new#main-content" });
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  let logouts = 0;
  try {
    await f.page.context().route(`${engineUrl}/auth/logout`, async route => {
      assert.equal(route.request().method(), "POST");
      logouts++;
      await pending;
      await route.fulfill({ headers: {
        "access-control-allow-origin": new URL(baseUrl).origin,
        "access-control-allow-credentials": "true", "cache-control": "no-store",
      }, json: { ok: true } });
    });
    await f.page.getByRole("button", { name: "退出登录", exact: true }).click();
    await f.page.getByRole("button", { name: "正在退出…", exact: true }).waitFor();
    await f.page.locator('nav a[href="/tasks/new"]').click();
    await f.page.waitForURL(`${baseUrl}/tasks/new`);
    release();
    await f.page.waitForURL(`${baseUrl}/login`, { timeout: 5000 });
    assert.equal(logouts, 1);
    assert.deepEqual(f.errors, []);
  } finally { release(); await f.close(); }
});

test("login redirects retain the task-page fragment when the initial session is rejected", { timeout: 30000 }, async () => {
  const f = await setupTask(browser, { path: "/tasks/new#main-content" });
  try {
    await f.page.context().route(`${engineUrl}/auth/me`, route => route.fulfill({ headers: {
      "access-control-allow-origin": new URL(baseUrl).origin,
      "access-control-allow-credentials": "true", "cache-control": "no-store",
    }, json: { authenticated: false } }));
    await f.page.reload();
    await f.page.waitForURL(url => url.pathname === "/login");
    assert.equal(new URL(f.page.url()).searchParams.get("next"), "/tasks/new#main-content");
    assert.equal(await f.page.getByTestId("task-title").count(), 0);
    await f.assertLocalOnly();
  } finally { await f.close(); }
});
