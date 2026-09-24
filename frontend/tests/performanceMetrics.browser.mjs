import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const stub = fileURLToPath(new URL("./fixtures/eventPageStubs.mjs", import.meta.url));
  bundle = await buildFixture(new URL("./fixtures/performanceMetrics.mjs", import.meta.url), {
    "next/router$": stub, "next/link$": stub, "next/head$": stub,
    [fileURLToPath(new URL("../src/utils/pageState.ts", import.meta.url))]: stub,
  });
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const operation = (total_requests = 17) => ({ total_requests, cache_hits: total_requests, cache_misses: 0,
  errors: 0, rejected: 0, in_flight: 0, in_flight_peak: 1, avg_duration_ms: 10 });
const metrics = (total = 17) => ({ check: operation(total), completion: operation(0) });

async function run(callback) {
  const f = await setupFixture(browser, bundle); f.page.setDefaultTimeout(2500);
  f.mount = async (options = {}) => { await f.page.evaluate(options => fixture.render(options), options); await f.advance(1); };
  f.ids = async () => (await f.requests()).flatMap((r, i) => new URL(r.url).pathname === "/settings/performance" ? [i] : []);
  f.latest = async () => (await f.ids()).at(-1);
  f.panel = f.page.locator("section").filter({ has: f.page.getByRole("heading", { name: "Backend Performance Metrics", exact: true }) }).last();
  f.button = () => f.page.getByRole("button", { name: "Refresh metrics", exact: true });
  f.reply = async (data, status = 200) => { await f.respond(await f.latest(), data, status); await f.advance(1); };
  f.waitSample = () => f.panel.getByText("Hotspot summary", { exact: true }).waitFor();
  f.state = () => f.page.getByTestId("hook-state").evaluate(el => JSON.parse(el.textContent));
  f.waitIdle = () => f.page.waitForFunction(() => fixture.metrics.refreshing === false, null, { polling: 10 });
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}

test("malformed metrics cannot crash the settings form or Runner panel", () => run(async f => {
  await f.mount(); await f.reply({ check: null, completion: operation() });
  assert.equal(await f.page.getByRole("button", { name: "Save Settings", exact: true }).count(), 1);
  assert.equal(await f.page.getByRole("heading", { name: "Runner Agent Operations", exact: true }).count(), 1);
}));

test("a stalled metrics read has a deadline and releases explicit refresh", () => run(async f => {
  await f.mount(); const index = await f.latest(); await f.advance(10010);
  assert.equal((await f.requests())[index].aborted, true);
  await f.button().click(); await f.advance(1);
  assert.ok((await f.ids()).length >= 2);
}));

test("navigation invalidates metrics and aborts obsolete reads", () => run(async f => {
  await f.mount(); const old = await f.latest();
  await f.mount({ navigation: "/settings?scope=next" });
  assert.equal((await f.requests())[old].aborted, true);
  assert.notEqual(await f.latest(), old);
}));

test("background failure marks retained metrics stale and removes old refresh success", () => run(async f => {
  await f.mount({ mode: "hook" }); await f.reply(metrics()); await f.waitSample();
  await f.page.evaluate(() => { void fixture.metrics.refresh(); }); await f.reply(metrics(21));
  await f.page.waitForFunction(() => fixture.metrics.message !== "");
  await f.advance(10010); await f.reply({ message: "private fixture detail" }, 503);
  const state = await f.state();
  assert.notEqual(state.error, ""); assert.equal(state.message, ""); assert.equal(state.stale, true);
  assert.equal(state.metrics.check.total_requests, 21);
}));

test("locale changes do not reload metrics or discard the settings draft", () => run(async f => {
  await f.mount(); await f.reply(metrics()); await f.waitSample();
  await f.page.getByRole("spinbutton").fill("717"); const count = (await f.ids()).length;
  await f.page.evaluate(() => fixture.setLocale("es")); await f.advance(1);
  assert.equal((await f.ids()).length, count);
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "717");
}));

test("saving settings cannot erase an unrelated metrics failure", () => run(async f => {
  await f.mount(); await f.reply(metrics()); await f.waitSample();
  await f.button().click(); await f.reply({}, 503);
  await f.page.locator(".error").first().waitFor();
  await f.page.getByRole("button", { name: "Save Settings", exact: true }).click();
  await f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ }).click();
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.ok(await f.page.locator(".error").count() > 0);
}));

test("pending event settings do not block independent metrics refresh", () => run(async f => {
  await f.mount({ holdEvents: true }); await f.reply({}, 503);
  assert.equal(await f.button().isDisabled(), false);
  assert.equal(await f.page.getByRole("button", { name: "Save Settings", exact: true }).isDisabled(), true);
}));

test("Strict Mode owns a new read and late obsolete responses cannot overwrite it", () => run(async f => {
  await f.mount({ mode: "hook" });
  const ids = await f.ids(); assert.equal(ids.length, 2);
  assert.equal((await f.requests())[ids[0]].aborted, true);
  await f.reply(metrics(30)); await f.waitIdle();
  await f.respond(ids[0], metrics(99)); await f.advance(1);
  assert.equal((await f.state()).metrics.check.total_requests, 30);
  for (const request of await f.requests()) {
    assert.equal(request.method, "GET"); assert.equal(request.body, undefined);
    assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error"); assert.equal(request.credentials, "include");
  }
}));

test("Engine changes hide the previous sample and invalidate old refresh callbacks", () => run(async f => {
  await f.mount({ mode: "hook" }); await f.reply(metrics()); await f.waitIdle();
  await f.page.evaluate(() => { fixture.oldRefresh = fixture.metrics.refresh; void fixture.metrics.refresh(); });
  const old = await f.latest();
  await f.mount({ engineUrl: "https://engine-b.invalid" });
  assert.equal((await f.state()).metrics, null);
  const count = (await f.ids()).length;
  await f.page.evaluate(() => fixture.oldRefresh());
  assert.equal((await f.ids()).length, count);
  assert.equal((await f.requests())[old].aborted, true);
  assert.equal(new URL((await f.requests())[await f.latest()].url).origin, "https://engine-b.invalid");
  await f.reply(metrics(40)); await f.waitIdle();
  await f.respond(old, metrics(99)); await f.advance(1);
  assert.equal((await f.state()).metrics.check.total_requests, 40);
}));

test("unmount stops metrics reads, late results and future polling", () => run(async f => {
  await f.mount({ mode: "hook" }); const last = await f.latest();
  await f.page.evaluate(() => fixture.render(false));
  const count = (await f.ids()).length;
  await f.respond(last, metrics()); await f.advance(40000);
  assert.equal((await f.requests())[last].aborted, true);
  assert.equal((await f.ids()).length, count);
  assert.equal(await f.page.getByTestId("hook-state").count(), 0);
}));

test("polling waits after completion and coalesces overlapping refresh requests", () => run(async f => {
  await f.mount({ mode: "hook" }); const initial = (await f.ids()).length;
  await f.advance(7000);
  await f.page.evaluate(() => { void fixture.metrics.refresh(); void fixture.metrics.refresh(); });
  assert.equal((await f.ids()).length, initial);
  await f.reply(metrics()); await f.waitIdle();
  await f.advance(9000); assert.equal((await f.ids()).length, initial);
  await f.advance(1010); assert.equal((await f.ids()).length, initial + 1);
  await f.page.evaluate(() => { void fixture.metrics.refresh(); void fixture.metrics.refresh(); });
  assert.equal((await f.ids()).length, initial + 1);
  await f.reply({}, 503); await f.waitIdle();
  await f.advance(9000); assert.equal((await f.ids()).length, initial + 1);
  await f.advance(1010); assert.equal((await f.ids()).length, initial + 2);
}));

test("a hung JSON body times out and cannot replace a later successful sample", () => run(async f => {
  await f.mount({ mode: "hook" }); const old = await f.latest();
  await f.page.evaluate(index => fixture.requests[index].resolve({
    ok: true, headers: new Headers({ "content-type": "application/json" }),
    json: () => new Promise(resolve => { fixture.releaseBody = resolve; }),
  }), old);
  await f.advance(10010); await f.waitIdle();
  assert.notEqual((await f.state()).error, "");
  assert.equal((await f.state()).metrics, null);
  await f.page.evaluate(() => { void fixture.metrics.refresh(); });
  await f.reply(metrics(60)); await f.waitIdle();
  await f.page.evaluate(value => fixture.releaseBody(value), metrics(99)); await f.advance(1);
  assert.equal((await f.state()).metrics.check.total_requests, 60);
}));

test("stale samples lose the healthy label until a validated retry succeeds", () => run(async f => {
  await f.mount({ mode: "hook" }); await f.reply(metrics()); await f.waitIdle();
  await f.page.evaluate(() => { void fixture.metrics.refresh(); });
  await f.reply({ message: "private fixture detail" }, 503); await f.waitIdle();
  assert.match(await f.panel.textContent(), /last successful readings/);
  assert.doesNotMatch(await f.panel.textContent(), /private fixture detail/);
  assert.equal(await f.panel.getByText("Safe", { exact: true }).count(), 0);
  await f.page.evaluate(() => { void fixture.metrics.refresh(); });
  assert.equal((await f.state()).stale, true);
  await f.reply(metrics(0)); await f.waitIdle();
  assert.equal((await f.state()).stale, false); assert.equal((await f.state()).error, "");
  assert.equal(await f.panel.getByText("Safe", { exact: true }).count(), 3);
}));

test("metrics failures leave Runner cancellation and settings controls independent", () => run(async f => {
  await f.mount(); await f.reply({ check: null });
  await f.panel.locator(".error").waitFor();
  assert.equal(await f.page.getByRole("button", { name: "Save Settings", exact: true }).isDisabled(), false);
  await f.page.getByRole("button", { name: "Cancel", exact: true }).click();
  assert.deepEqual(await f.page.evaluate(() => fixture.writes), []);
  await f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ }).click();
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.deepEqual(await f.page.evaluate(() => fixture.writes), [{ path: "/runner/jobs/cancel", body: { job_id: "fixture-job" } }]);
  assert.equal(await f.panel.locator(".error").count(), 1);
  await f.button().click(); await f.reply(metrics()); await f.waitSample();
  assert.equal((await f.page.evaluate(() => fixture.writes)).length, 1);
}));

test("unavailable and stale feedback translates without extra reads in all four locales", () => run(async f => {
  await f.mount({ mode: "hook" }); await f.reply(metrics()); await f.waitIdle();
  await f.page.evaluate(() => { void fixture.metrics.refresh(); }); await f.reply({}, 503); await f.waitIdle();
  const count = (await f.ids()).length;
  for (const [locale, text] of [["zh-CN", "未能刷新性能指标"], ["es", "No se pudieron"], ["ja", "性能指標を更新できませんでした"], ["en", "Could not refresh performance metrics"]]) {
    await f.page.evaluate(locale => fixture.setLocale(locale), locale); await f.advance(1);
    assert.ok((await f.state()).error.includes(text));
    assert.equal((await f.state()).stale, true);
  }
  assert.equal((await f.ids()).length, count);
}));
