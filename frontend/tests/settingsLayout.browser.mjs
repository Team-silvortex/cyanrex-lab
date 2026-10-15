import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
import { baseUrl, engineUrl, setup } from "./helpers/actionSafetyBrowser.mjs";

const localeRoot = new URL("../src/i18n/", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, next) {
  return context.parentURL?.startsWith(localeRoot) && specifier.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(specifier)
    ? next(new URL(`${specifier}.ts`, context.parentURL).href, context) : next(specifier, context);
} });
let translations;
try { ({ translations } = await import("../src/i18n/translations.ts")); }
finally { resolver.deregister(); }
const text = (locale, key, variables = {}) => {
  const value = key.split(".").reduce((dict, part) => dict?.[part], translations[locale]);
  assert.equal(typeof value, "string", `${locale}.${key} is explicit`);
  return value.replace(/\{(\w+)\}/g, (_, name) => String(variables[name]));
};
const profile = { id: "primary", name: "Team assistant", protocol: "openai_responses",
  base_url: "https://provider.invalid/v1", model: "chosen-by-operator", credential_ref: "TEAM_AI_KEY", enabled: true };
const settings = { revision: 3, default_profile_id: "primary", profiles: [profile] };
const operation = { total_requests: 17, cache_hits: 15, cache_misses: 2, errors: 0,
  rejected: 0, in_flight: 0, in_flight_peak: 1, avg_duration_ms: 2 };
const metrics = { check: operation, completion: { ...operation, total_requests: 23 } };
const allowedPaths = new Set(["/auth/me", "/events/unread-count", "/settings/events", "/settings/compiler",
  "/settings/performance", "/settings/ai-agents", "/runner/agents", "/runner/jobs"]);
const jsonHeaders = { "access-control-allow-origin": new URL(baseUrl).origin,
  "access-control-allow-credentials": "true", "cache-control": "no-store" };

async function fixture({ failedEvents = false } = {}) {
  const f = await setup("teacher");
  f.page.setDefaultTimeout(10000);
  f.requests = []; f.expectedWrites = [];
  f.view = { metricsFail: false };
  f.page.on("request", request => {
    const url = new URL(request.url());
    if (url.origin === new URL(engineUrl).origin) f.requests.push({
      path: url.pathname, method: request.method(), search: url.search,
      bytes: request.postDataBuffer()?.length || 0,
    });
  });
  try {
    await f.page.context().route(`${engineUrl}/settings/ai-agents`, route => {
      const request = route.request(), url = new URL(request.url());
      if (request.method() !== "GET" || url.search || request.postData()) return route.fallback();
      return route.fulfill({ headers: jsonHeaders, json: settings });
    });
    await f.page.context().route(`${engineUrl}/settings/performance`, route => {
      const request = route.request(), url = new URL(request.url());
      if (request.method() !== "GET" || url.search || request.postData()) return route.fallback();
      return route.fulfill({ headers: jsonHeaders, status: f.view.metricsFail ? 503 : 200,
        json: f.view.metricsFail ? { ok: false } : metrics });
    });
    if (failedEvents) await f.page.context().route(`${engineUrl}/settings/events`, route => {
      const request = route.request(), url = new URL(request.url());
      if (request.method() !== "GET" || url.search || request.postData()) return route.fallback();
      return route.fulfill({ headers: jsonHeaders, status: 503, json: { ok: false } });
    });
    await f.page.goto(`${baseUrl}/settings`);
    await f.page.getByRole("heading", { name: text("zh-CN", "settings.title"), exact: true }).waitFor();
    await f.page.getByRole("textbox", { name: text("zh-CN", "aiAgents.name"), exact: true }).waitFor();
    if (failedEvents) await f.page.getByText(text("zh-CN", "settings.loadFailed"), { exact: true }).waitFor();
    else await f.page.locator('input[type="number"]:enabled').waitFor();
    await f.page.getByText(text("zh-CN", "settings.hotspotSummary"), { exact: true }).waitFor();
    f.form = f.page.getByTestId("settings-form");
    f.actions = f.page.getByTestId("settings-form-actions");
    f.performance = f.page.locator(".settings-performance");
    f.documentation = f.page.getByTestId("settings-documentation");
    f.max = locale => f.form.getByRole("spinbutton", { name: text(locale, "settings.maxRecords"), exact: true });
    f.resident = locale => f.form.getByRole("checkbox", { name: text(locale, "settings.residentCompiler"), exact: true });
    f.ai = locale => f.page.getByRole("region", { name: text(locale, "aiAgents.title"), exact: true });
    f.aiName = locale => f.ai(locale).getByRole("textbox", { name: text(locale, "aiAgents.name"), exact: true });
    f.save = locale => f.actions.getByRole("button", { name: text(locale, "settings.save"), exact: true });
    f.aiSave = locale => f.ai(locale).getByRole("button", { name: text(locale, "aiAgents.save"), exact: true });
    f.refresh = locale => f.performance.getByRole("button", { name: text(locale, "settings.refreshMetrics"), exact: true });
    return f;
  } catch (error) {
    const failures = [error];
    try {
      assert.deepEqual(f.errors, [], "initialization has no page error or unexpected request");
      assert.deepEqual(f.writes, [], "initialization never writes");
    } catch (isolation) { failures.push(isolation); }
    finally { try { await f.browser.close(); } catch (cleanup) { failures.push(cleanup); } }
    throw failures.length === 1 ? error : new AggregateError(failures, "Settings initialization and isolation failed");
  }
}
function settingsTest(name, body, options) {
  test(name, { timeout: 90000 }, async () => {
    const f = await fixture(options);
    let failure;
    const record = error => { failure = failure ? new AggregateError([failure, error], "Settings behavior and isolation failed") : error; };
    try { await body(f); } catch (error) { record(error); }
    finally {
      try { assert.equal(await f.page.evaluate(() => window.fixtureSockets.length), 0, "settings never starts a kernel/event stream"); }
      catch (error) { record(error); }
      finally { try { await f.browser.close(); } catch (error) { record(error); } }
      try {
        assert.deepEqual(f.errors, [], "no page error or unexpected external/API request");
        assert.deepEqual(f.writes, f.expectedWrites, "no implicit save, AI transport, probe or cancellation");
        assert.ok(f.requests.length > 0);
        for (const request of f.requests) {
          assert.ok(allowedPaths.has(request.path), request.path);
          assert.equal(request.search, "", "layout introduces no query or navigation scope");
          assert.ok(["GET", "OPTIONS", "POST"].includes(request.method));
          if (request.method !== "POST") assert.equal(request.bytes, 0);
        }
      } catch (error) { record(error); }
    }
    if (failure) throw failure;
  });
}
const readCount = (f, path) => f.requests.filter(request => request.path === path && request.method === "GET").length;
async function fitAndHit(locator, label) {
  await locator.scrollIntoViewIfNeeded();
  const r = await locator.evaluate(element => {
    const box = element.getBoundingClientRect();
    return { x: box.x, right: box.right, height: box.height, width: box.width, viewport: innerWidth,
      hit: element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)) };
  });
  assert.ok(r.width > 0 && r.height >= 40, `${label} is at least 40px high`);
  assert.ok(r.x >= 0 && r.right <= r.viewport + 1, `${label} fits horizontally`);
  assert.equal(r.hit, true, `${label} is not obscured`);
}
async function changeLocale(f, locale) {
  const menu = f.page.locator(".menu-toggle");
  if (await menu.isVisible()) await menu.click();
  await f.page.locator(".sidebar-footer select").selectOption(locale);
  await f.page.waitForFunction(locale => document.documentElement.lang === locale, locale);
  if (await menu.isVisible()) {
    await f.page.locator(".sidebar-footer select").focus();
    await f.page.keyboard.press("Escape");
    assert.equal(await menu.getAttribute("aria-expanded"), "false");
  }
}
async function cancelReview(f, action, locale) {
  await action.click();
  const dialog = f.page.getByRole("dialog", { name: text(locale, "safety.title"), exact: true });
  await dialog.waitFor();
  await dialog.getByRole("button", { name: text(locale, "safety.cancel"), exact: true }).click();
  await dialog.waitFor({ state: "detached" });
}

settingsTest("settings cards place local save actions after compiler fields and separate desktop metrics from mobile controls", async f => {
  assert.equal(await f.page.locator(".settings-workspace").count(), 1);
  assert.equal(await f.form.count(), 1); assert.equal(await f.actions.count(), 1);
  assert.equal(await f.form.getAttribute("aria-label"), text("zh-CN", "settings.title"));
  assert.equal(await f.performance.getAttribute("aria-label"), text("zh-CN", "settings.performanceTitle"));
  assert.equal(await f.actions.getByRole("button").count(), 2);
  assert.equal(await f.form.getByRole("button", { name: text("zh-CN", "settings.refreshMetrics"), exact: true }).count(), 0);
  assert.equal(await f.form.locator('a[href^="/learn"]').count(), 0);
  const order = await f.actions.evaluate(element => {
    const compiler = document.querySelector('[data-testid="settings-form"] input[type="checkbox"]');
    const docs = document.querySelector('[data-testid="settings-documentation"]');
    const management = document.querySelector(".settings-management");
    return { afterCompiler: Boolean(compiler.compareDocumentPosition(element) & Node.DOCUMENT_POSITION_FOLLOWING),
      beforeDocs: Boolean(element.compareDocumentPosition(docs) & Node.DOCUMENT_POSITION_FOLLOWING),
      docsAfterManagement: Boolean(management.compareDocumentPosition(docs) & Node.DOCUMENT_POSITION_FOLLOWING) };
  });
  assert.deepEqual(order, { afterCompiler: true, beforeDocs: true, docsAfterManagement: true });
  assert.equal(await f.ai("zh-CN").evaluate(element => Boolean(element.closest(".settings-management"))), true);
  assert.equal(await f.page.locator(".settings-management").getByRole("region", {
    name: text("zh-CN", "settings.runnerTitle"), exact: true,
  }).count(), 1);
  await f.page.setViewportSize({ width: 1440, height: 900 });
  const form = await f.form.boundingBox(), performance = await f.performance.boundingBox();
  assert.ok(form && performance && form.width > 0 && performance.width > 0);
  assert.ok(form.x + form.width <= performance.x + 1, "desktop form and metrics are distinct side-by-side cards");
  assert.ok(Math.abs(form.y - performance.y) <= 2);
  for (const width of [390, 320]) {
    await f.page.setViewportSize({ width, height: 900 });
    await f.page.evaluate(() => scrollTo(0, 0));
    const a = await f.form.boundingBox(), b = await f.performance.boundingBox();
    assert.ok(a.y + a.height <= b.y + 1, "mobile cards stack without intersection");
    const save = await f.save("zh-CN").boundingBox();
    assert.ok(save.y >= 0 && save.y + save.height <= 620, "local save stays beside its fields in the first mobile screen");
    assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    for (const [control, label] of [[f.max("zh-CN"), "retention"], [f.form.getByRole("combobox"), "overflow policy"],
      [f.save("zh-CN"), "save"], [f.actions.getByRole("button", { name: text("zh-CN", "settings.reload"), exact: true }), "reload"],
      [f.refresh("zh-CN"), "metrics refresh"]]) await fitAndHit(control, `${width}px ${label}`);
  }
});

settingsTest("resize metrics refresh and closed-menu locale changes keep unsaved event compiler and AI drafts in one scope", async f => {
  await f.max("zh-CN").fill("700"); await f.resident("zh-CN").check();
  await f.aiName("zh-CN").fill("Unsaved operator name");
  const url = f.page.url(), originalPath = await f.page.evaluate(() => window.next.router.asPath);
  const counts = ["/settings/events", "/settings/compiler", "/settings/ai-agents"].map(path => readCount(f, path));
  const aiElement = await f.ai("zh-CN").elementHandle();
  try {
    for (const [width, locale] of [[1440, "en"], [960, "es"], [390, "ja"], [320, "ko"], [1440, "zh-CN"]]) {
      await f.page.setViewportSize({ width, height: 900 });
      await changeLocale(f, locale);
      await f.refresh(locale).click();
      await f.performance.getByText(text(locale, "settings.metricsUpdated"), { exact: true }).waitFor();
      assert.equal(await f.max(locale).inputValue(), "700"); assert.equal(await f.resident(locale).isChecked(), true);
      assert.equal(await f.aiName(locale).inputValue(), "Unsaved operator name");
      assert.equal(await aiElement.evaluate(element => element.isConnected), true, "management is not conditionally unmounted");
      assert.equal(f.page.url(), url); assert.equal(await f.page.evaluate(() => window.next.router.asPath), originalPath);
      assert.deepEqual(["/settings/events", "/settings/compiler", "/settings/ai-agents"].map(path => readCount(f, path)), counts);
      assert.deepEqual(f.writes, []);
    }
    await cancelReview(f, f.save("zh-CN"), "zh-CN");
    await cancelReview(f, f.aiSave("zh-CN"), "zh-CN");
    assert.equal(await f.max("zh-CN").inputValue(), "700"); assert.equal(await f.resident("zh-CN").isChecked(), true);
    assert.equal(await f.aiName("zh-CN").inputValue(), "Unsaved operator name");
  } finally { await aiElement.dispose(); }
});

settingsTest("confirmed event saves write only events and compiler while independent AI confirmation writes only reviewed metadata", async f => {
  await f.max("zh-CN").fill("700"); await f.resident("zh-CN").check();
  await f.aiName("zh-CN").fill("Reviewed operator name");
  const eventWrite = { path: "/settings/events", params: {}, body: { max_records: 700, overflow_policy: "drop_oldest" } };
  const compilerWrite = { path: "/settings/compiler", params: {}, body: { resident: true } };
  f.expectedWrites = [eventWrite, compilerWrite];
  await f.save("zh-CN").click();
  let dialog = f.page.getByRole("dialog", { name: text("zh-CN", "safety.title"), exact: true });
  await dialog.getByText("700", { exact: true }).waitFor();
  assert.deepEqual(f.writes, []);
  await dialog.getByRole("button", { name: text("zh-CN", "safety.confirm", { action: text("zh-CN", "settings.save") }), exact: true }).click();
  await dialog.waitFor({ state: "detached" });
  await f.form.getByText(text("zh-CN", "settings.saved"), { exact: true }).waitFor();
  assert.deepEqual(f.writes, [eventWrite, compilerWrite]);
  assert.equal(await f.aiName("zh-CN").inputValue(), "Reviewed operator name");
  assert.equal(await f.aiSave("zh-CN").isEnabled(), true);
  await cancelReview(f, f.aiSave("zh-CN"), "zh-CN");
  assert.deepEqual(f.writes, [eventWrite, compilerWrite]);
  const body = { expected_revision: 3, default_profile_id: "primary", profiles: [{ ...profile, name: "Reviewed operator name" }] };
  f.expectedWrites = [eventWrite, compilerWrite, { path: "/settings/ai-agents", params: {}, body }];
  await f.page.context().route(`${engineUrl}/settings/ai-agents`, route => {
    const request = route.request(), url = new URL(request.url());
    if (request.method() !== "POST" || url.search) return route.fallback();
    const submitted = request.postDataJSON();
    f.writes.push({ path: url.pathname, params: Object.fromEntries(url.searchParams), body: submitted });
    return route.fulfill({ headers: jsonHeaders, json: { ok: true, settings: { revision: 4,
      default_profile_id: submitted.default_profile_id, profiles: submitted.profiles } } });
  });
  await f.aiSave("zh-CN").click();
  dialog = f.page.getByRole("dialog", { name: text("zh-CN", "safety.title"), exact: true });
  await dialog.getByText(engineUrl, { exact: true }).waitFor();
  await dialog.getByRole("button", { name: text("zh-CN", "safety.confirm", { action: text("zh-CN", "aiAgents.save") }), exact: true }).click();
  await dialog.waitFor({ state: "detached" });
  await f.ai("zh-CN").getByText(text("zh-CN", "aiAgents.saved"), { exact: true }).waitFor();
  assert.equal(await f.aiSave("zh-CN").isDisabled(), true);
  assert.equal(await f.max("zh-CN").inputValue(), "700"); assert.equal(await f.resident("zh-CN").isChecked(), true);
  assert.deepEqual(f.writes, f.expectedWrites);
});

settingsTest("all five narrow-screen locales keep failed event saving blocked stale metrics explicit and AI cancellation reachable", async f => {
  await f.aiName("zh-CN").fill("Keep this unsaved AI name");
  f.view.metricsFail = true;
  await f.refresh("zh-CN").click();
  await f.performance.getByText(text("zh-CN", "settings.metricsStale"), { exact: true }).waitFor();
  const originalUrl = f.page.url();
  const expectedLinks = ["/learn", "/learn/troubleshooting", "/learn/teacher-guide", "/learn/student-guide", "/learn/concepts", "/learn/labs/01-first-program"];
  for (const locale of ["zh-CN", "en", "es", "ja", "ko"]) {
    await f.page.setViewportSize({ width: 390, height: 900 }); await changeLocale(f, locale);
    for (const width of [390, 320]) {
      await f.page.setViewportSize({ width, height: 900 });
      assert.equal(await f.max(locale).isDisabled(), true); assert.equal(await f.save(locale).isDisabled(), true);
      await f.form.getByText(text(locale, "settings.loadFailed"), { exact: true }).waitFor();
      await f.performance.getByText(text(locale, "settings.metricsReadFailed"), { exact: true }).waitFor();
      await f.performance.getByText(text(locale, "settings.metricsStale"), { exact: true }).waitFor();
      assert.equal(await f.aiName(locale).inputValue(), "Keep this unsaved AI name");
      assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      await fitAndHit(f.refresh(locale), `${locale} metrics refresh`);
      await fitAndHit(f.actions.getByRole("button", { name: text(locale, "settings.reload"), exact: true }), `${locale} reload`);
      assert.deepEqual(await f.documentation.locator("a").evaluateAll(links => links.map(link => link.getAttribute("href"))), expectedLinks);
      for (const href of expectedLinks) assert.equal(new URL(href, baseUrl).hash, "");
      assert.doesNotMatch(await f.page.locator("main").innerText(), /\b(?:settings|aiAgents|safety|layout)\.[A-Za-z][\w.]*/);
      await f.aiSave(locale).click();
      const dialog = f.page.getByRole("dialog", { name: text(locale, "safety.title"), exact: true });
      await dialog.waitFor();
      const cancel = dialog.getByRole("button", { name: text(locale, "safety.cancel"), exact: true });
      await fitAndHit(cancel, `${locale} ${width}px confirmation cancel`);
      const geometry = await dialog.boundingBox();
      assert.ok(geometry.x >= 0 && geometry.x + geometry.width <= width + 1);
      assert.equal(await f.aiName(locale).isDisabled(), true);
      await cancel.click(); await dialog.waitFor({ state: "detached" });
      assert.equal(await f.aiName(locale).inputValue(), "Keep this unsaved AI name");
      assert.equal(await f.aiSave(locale).isEnabled(), true);
      assert.equal(f.page.url(), originalUrl); assert.deepEqual(f.writes, []);
    }
  }
}, { failedEvents: true });
