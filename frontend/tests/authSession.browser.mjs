import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";

async function setup() {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  await context.addInitScript(() => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    window.logoutSettled = 0;
    const originalFetch = window.fetch;
    window.fetch = async (...args) => {
      try { return await originalFetch(...args); }
      finally { if (String(args[0]).endsWith("/auth/logout")) window.logoutSettled += 1; }
    };
  });
  const page = await context.newPage(), errors = [], writes = [];
  const state = { status: 403, ok: false, networkFailure: false, hold: null, authenticated: true };
  page.on("pageerror", error => errors.push(error.message));
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === new URL(baseUrl).origin) return route.continue();
    errors.push("Blocked an unmocked external request");
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, async route => {
    const request = route.request(), path = new URL(request.url()).pathname;
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type", "cache-control": "no-store" };
    const reply = (json, status = 200) => route.fulfill({ json, status, headers });
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    if (path === "/auth/me") return reply({ authenticated: state.authenticated, username: "teacher", role: "teacher" });
    if (path === "/events/unread-count") return reply({ unread: 0 });
    if (path === "/auth/logout" && request.method() === "POST") {
      writes.push(path);
      if (state.hold) await state.hold;
      if (state.networkFailure) return route.abort("failed");
      if (state.status === 200 && state.ok) state.authenticated = false;
      return reply({ ok: state.ok, message: "synthetic logout result" }, state.status);
    }
    errors.push(`Unexpected API ${request.method()} ${path}`);
    return reply({}, 404);
  });
  await page.goto(`${baseUrl}/account`);
  await page.getByRole("button", { name: "退出登录", exact: true }).waitFor();
  return { browser, page, state, writes, errors };
}

test("failed logout stays on the current page, reports uncertainty and permits an explicit retry", { timeout: 30000 }, async () => {
  const f = await setup();
  try {
    for (const failure of [{ status: 403, ok: false }, { status: 503, ok: false }, { status: 200, ok: false }, { networkFailure: true }]) {
      Object.assign(f.state, failure);
      const before = f.writes.length;
      await f.page.getByRole("button", { name: "退出登录", exact: true }).click();
      await f.page.getByRole("alert").filter({ hasText: "无法确认退出成功" }).waitFor({ timeout: 2000 });
      assert.equal(new URL(f.page.url()).pathname, "/account");
      assert.equal(f.writes.length, before + 1, "logout is never automatically retried");
      if (before === 0) {
        await f.page.setViewportSize({ width: 320, height: 844 });
        await f.page.locator(".menu-toggle").click();
        for (const locale of ["en", "es", "ja", "zh-CN"]) {
          await f.page.locator(".sidebar-footer select").selectOption(locale);
          const warning = f.page.locator(".sidebar-footer [role=alert]");
          await warning.waitFor();
          assert.equal(await warning.evaluate(element => element.scrollWidth <= element.clientWidth), true);
          assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
          assert.equal((await warning.textContent()).includes("layout.logoutFailed"), false);
        }
        await f.page.setViewportSize({ width: 1280, height: 900 });
      }
    }
    Object.assign(f.state, { status: 200, ok: true, networkFailure: false });
    await f.page.getByRole("button", { name: "退出登录", exact: true }).click();
    await f.page.waitForURL(`${baseUrl}/login`);
    assert.equal(f.writes.length, 5);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("logout ignores duplicate clicks while pending and does not redirect until confirmed", { timeout: 30000 }, async () => {
  const f = await setup();
  let release;
  try {
    Object.assign(f.state, { status: 200, ok: true, hold: new Promise(resolve => { release = resolve; }) });
    await f.page.getByRole("button", { name: "退出登录", exact: true }).evaluate(button => { button.click(); button.click(); });
    await f.page.getByRole("button", { name: "正在退出…", exact: true }).waitFor({ timeout: 2000 });
    assert.equal(await f.page.getByRole("button", { name: "正在退出…", exact: true }).isDisabled(), true);
    assert.equal(new URL(f.page.url()).pathname, "/account");
    release();
    await f.page.waitForURL(`${baseUrl}/login`);
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { release?.(); await f.browser.close(); }
});

test("navigating away discards late logout UI callbacks without claiming server rollback", { timeout: 30000 }, async () => {
  const f = await setup();
  let release;
  try {
    Object.assign(f.state, { status: 200, ok: true, hold: new Promise(resolve => { release = resolve; }) });
    await f.page.evaluate(() => {
      window.loginRedirects = 0;
      const replace = window.next.router.replace.bind(window.next.router);
      window.next.router.replace = (...args) => {
        if (args[0] === "/login") window.loginRedirects += 1;
        return replace(...args);
      };
    });
    await f.page.getByRole("button", { name: "退出登录", exact: true }).click();
    await f.page.getByRole("button", { name: "正在退出…", exact: true }).waitFor({ timeout: 2000 });
    await f.page.locator('nav a[href="/dashboard"]').click();
    await f.page.waitForURL(`${baseUrl}/dashboard`);
    await f.page.getByRole("button", { name: "退出登录", exact: true }).waitFor();
    release();
    await f.page.waitForFunction(() => window.logoutSettled === 1);
    assert.equal(await f.page.evaluate(() => window.loginRedirects), 0);
    assert.equal(new URL(f.page.url()).pathname, "/dashboard");
    assert.equal(await f.page.getByRole("alert").filter({ hasText: "无法确认退出成功" }).count(), 0);
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { release?.(); await f.browser.close(); }
});
