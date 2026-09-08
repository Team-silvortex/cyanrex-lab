import assert from "node:assert/strict";
import test from "node:test";

// Optional browser smoke against a production frontend. Engine HTTP and WS are local fixtures.
const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
const event = (index, severity = "success") => ({
  username: "student", timestamp: `2026-09-08T00:00:0${index}Z`, source: "browser-fixture",
  event_type: `fixture.${index}`, category: "kernel", severity,
  color: severity === "error" ? "red" : "green", payload: { index },
});

test("event page recovers visibly, preserves live rows across snapshots, and changes filters safely", { timeout: 60000 }, async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  try {
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    await context.addInitScript(() => {
      localStorage.setItem("cyanrex_locale", "zh-CN");
      window.fixtureSockets = [];
      window.WebSocket = class {
        closed = false;
        constructor() {
          window.fixtureSockets.push(this);
          setTimeout(() => { if (!this.closed) this.onopen?.({}); }, 0);
        }
        close() { this.closed = true; this.onclose?.({ code: 1000 }); }
      };
    });
    const page = await context.newPage();
    const errors = []; page.on("pageerror", (error) => errors.push(error.message));
    let snapshots = 0;
    const fixture = [event(1)];
    await context.route(`${engineUrl}/**`, async (route) => {
      const request = route.request(), url = new URL(request.url());
      const headers = {
        "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
        "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type",
      };
      const reply = (json) => route.fulfill({ json, headers });
      if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
      if (url.pathname === "/auth/me") return reply({ authenticated: true, username: "student", role: "student" });
      if (url.pathname === "/events/unread-count") return reply({ unread: 0 });
      if (url.pathname === "/events/mark-read") return reply({ ok: true });
      if (url.pathname === "/events") {
        snapshots++;
        const rows = fixture.filter((row) => !url.searchParams.get("severity") || row.severity === url.searchParams.get("severity"));
        // Deliver a live record while the reconnect snapshot is still in flight.
        if (snapshots === 2) await page.evaluate((row) => window.fixtureSockets.at(-1).onmessage({ data: JSON.stringify(row) }), event(3));
        return reply(rows);
      }
      throw Error(`Unexpected fixture request ${url.pathname}`);
    });
    await page.goto(`${baseUrl}/events`);
    await page.getByText("fixture.1", { exact: true }).waitFor();
    fixture.push(event(2));
    await page.evaluate(() => {
      const socket = window.fixtureSockets.at(-1);
      socket.closed = true; socket.onclose({ code: 1013 });
    });
    await page.getByRole("status").filter({ hasText: "部分事件可能缺失" }).waitFor();
    await page.getByText("fixture.3", { exact: true }).waitFor();
    for (const index of [1, 2, 3]) assert.equal(await page.getByText(`fixture.${index}`, { exact: true }).count(), 1);
    assert.equal(snapshots, 2);
    fixture.push(event(4, "error"));
    await page.locator("select").filter({ has: page.locator('option[value="error"]') }).selectOption("error");
    await page.getByText("fixture.4", { exact: true }).waitFor();
    assert.equal(await page.locator("article").count(), 1);
    await page.evaluate((row) => window.fixtureSockets.at(-1).onmessage({ data: JSON.stringify(row) }), event(5));
    assert.equal(await page.getByText("fixture.5", { exact: true }).count(), 0);
    assert.equal(await page.evaluate(() => window.fixtureSockets.filter((socket) => !socket.closed).length), 1);
    if (process.env.CYANREX_UI_SCREENSHOT) await page.screenshot({ path: process.env.CYANREX_UI_SCREENSHOT });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
