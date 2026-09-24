import assert from "node:assert/strict";
import test from "node:test";
import { baseUrl, setup } from "./helpers/actionSafetyBrowser.mjs";

const row = { username: "student", timestamp: "2026-09-13T00:00:00Z", source: "fixture",
  event_type: "fixture.event", category: "kernel", severity: "error", color: "red", payload: { value: "test only" } };

test("production event export downloads the verified JSON response without event mutations", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.events = [row]; await f.page.goto(`${baseUrl}/events`); await f.page.getByText("fixture.event", { exact: true }).waitFor();
    const downloaded = f.page.waitForEvent("download");
    await f.page.getByRole("button", { name: "导出下载", exact: true }).click();
    const download = await downloaded, chunks = [];
    for await (const chunk of await download.createReadStream()) chunks.push(chunk);
    assert.equal(await download.failure(), null); assert.equal(download.suggestedFilename(), "cyanrex-events-fixture.json");
    assert.deepEqual(JSON.parse(Buffer.concat(chunks).toString()), [row]); assert.equal(f.state.exports.length, 1);
    assert.equal(f.writes.length, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("invalid time ranges stay visible and cannot export or delete in all four narrow-screen locales", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.events = [row]; await f.page.goto(`${baseUrl}/events`); await f.page.getByText("fixture.event", { exact: true }).waitFor();
    await f.page.locator("main select").filter({ has: f.page.locator('option[value="custom"]') }).selectOption("custom");
    await f.page.locator('input[type="datetime-local"]').first().fill("2026-09-13T05:00");
    await f.page.locator('input[type="datetime-local"]').last().fill("2026-09-13T04:00");
    await f.page.setViewportSize({ width: 320, height: 844 });
    for (const locale of ["en", "zh-CN", "es", "ja"]) {
      await f.page.locator(".menu-toggle").click();
      await f.page.locator(".sidebar-footer select").selectOption(locale);
      await f.page.locator(".menu-toggle").click();
      assert.equal(await f.page.locator("main [role=alert]").count(), 1);
      assert.equal(await f.page.locator("main button.button-danger").isDisabled(), true);
      assert.equal(await f.page.locator("main select").filter({ has: f.page.locator('option[value="csv"]') })
        .locator("..").locator("..").getByRole("button").filter({ hasText: /Export|导出|エクスポート/ }).isDisabled(), true);
      assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    }
    assert.equal(f.writes.length, 0); assert.equal(f.state.exports?.length || 0, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("unknown unread counts and failed acknowledgements stay visible until an explicit successful retry", { timeout: 45000 }, async () => {
  const f = await setup("student");
  try {
    f.state.events = [row]; f.state.unreadResult = { unread: "7" }; f.state.markReadStatus = 503;
    await f.page.goto(`${baseUrl}/events`); await f.page.getByRole("button", { name: "重试已读标记", exact: true }).waitFor();
    assert.equal(await f.page.locator(".nav-badge").textContent(), "?");
    f.state.markReadStatus = 200; f.state.unreadResult = { unread: 0 };
    await f.page.getByRole("button", { name: "重试已读标记", exact: true }).click();
    await f.page.locator(".nav-badge").waitFor({ state: "detached" });
    assert.equal(await f.page.getByRole("button", { name: "重试已读标记", exact: true }).count(), 0);
    assert.equal(await f.page.locator("article").count(), 1); assert.equal(f.writes.length, 0); assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
