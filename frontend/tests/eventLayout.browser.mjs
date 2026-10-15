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
const START = "2000-01-01T00:00", END = "2000-01-04T23:59";
const row = { username: "teacher", timestamp: "2000-01-02T12:00:00Z", source: "original-" + "s".repeat(2048),
  event_type: "fixture.event-layout", category: "platform", severity: "warning", color: "yellow",
  payload: { source: "// Original local code is display data, not execution.",
    entries: Array.from({ length: 80 }, (_, index) => ({ index, word: "literal".repeat(160) })) } };
const csv = `timestamp,source,category,severity\n${row.timestamp},${row.source},platform,warning\n`;
const headers = { "access-control-allow-origin": new URL(baseUrl).origin,
  "access-control-allow-credentials": "true", "cache-control": "no-store" };
const allowedPaths = new Set(["/auth/me", "/events", "/events/unread-count", "/events/mark-read", "/events/export", "/events/delete"]);

async function fixture({ ackFailure = false } = {}) {
  const f = await setup("teacher");
  f.page.setDefaultTimeout(10000);
  f.state.events = [row]; f.state.markReadStatus = ackFailure ? 503 : 200;
  f.requests = []; f.exports = []; f.expectedWrites = []; f.expectedExports = [];
  f.page.on("request", request => {
    const url = new URL(request.url());
    if (url.origin === new URL(engineUrl).origin) f.requests.push({ path: url.pathname,
      method: request.method(), params: Object.fromEntries(url.searchParams), bytes: request.postDataBuffer()?.length || 0 });
  });
  try {
    await f.page.context().route(`${engineUrl}/events/export?*`, route => {
      const request = route.request(), url = new URL(request.url());
      if (request.method() !== "GET" || request.postData() || !["json", "csv"].includes(url.searchParams.get("format"))) return route.fallback();
      const format = url.searchParams.get("format");
      f.exports.push(Object.fromEntries(url.searchParams));
      return route.fulfill({ headers: { ...headers,
        "content-type": format === "json" ? "application/json" : "text/csv; charset=utf-8",
        "content-disposition": `attachment; filename="cyanrex-layout-fixture.${format}"`,
        "access-control-expose-headers": "content-disposition" },
        body: format === "json" ? JSON.stringify([row]) : csv });
    });
    await f.page.goto(`${baseUrl}/events`);
    await f.page.getByText(row.event_type, { exact: true }).waitFor();
    f.controls = f.page.getByTestId("events-controls");
    f.filters = f.page.getByTestId("events-filters");
    f.actions = f.page.getByTestId("events-actions");
    f.danger = f.page.getByTestId("events-danger-actions");
    f.list = f.page.getByTestId("events-list");
    f.select = (locale, key) => (key === "export" ? f.actions : f.filters)
      .getByRole("combobox", { name: text(locale, `events.${key}`), exact: true });
    f.date = (locale, key) => f.filters.getByLabel(text(locale, `events.${key}`), { exact: true });
    f.refresh = locale => f.actions.getByRole("button", { name: text(locale, "events.refreshHistory"), exact: true });
    f.export = locale => f.actions.getByRole("button", { name: text(locale, "events.exportDownload"), exact: true });
    f.delete = locale => f.danger.getByRole("button", { name: text(locale, "events.deleteFiltered"), exact: true });
    return f;
  } catch (error) {
    const failures = [error];
    try { assert.deepEqual(f.errors, []); assert.deepEqual(f.writes, []); assert.deepEqual(f.exports, []); }
    catch (isolation) { failures.push(isolation); }
    finally { try { await f.browser.close(); } catch (cleanup) { failures.push(cleanup); } }
    throw failures.length === 1 ? error : new AggregateError(failures, "Event initialization and isolation failed");
  }
}
function eventTest(name, body, options) {
  test(name, { timeout: 90000 }, async () => {
    const f = await fixture(options);
    let failure;
    const record = error => { failure = failure ? new AggregateError([failure, error], "Event layout and isolation failed") : error; };
    try { await body(f); } catch (error) { record(error); }
    finally {
      try {
        const sockets = await f.page.evaluate(() => window.fixtureSockets.map(socket => ({ url: socket.url, closed: socket.closed })));
        assert.ok(sockets.length > 0, "the original event stream is present");
        assert.ok(sockets.filter(socket => !socket.closed).length <= 1);
        for (const socket of sockets) {
          const url = new URL(socket.url);
          assert.equal(url.host, new URL(engineUrl).host); assert.equal(url.pathname, "/ws/events");
          assert.equal(url.search, ""); assert.ok(["ws:", "wss:"].includes(url.protocol));
        }
      } catch (error) { record(error); }
      finally { try { await f.browser.close(); } catch (error) { record(error); } }
      try {
        assert.deepEqual(f.errors, [], "no page error or unmocked API/provider request");
        assert.deepEqual(f.writes, f.expectedWrites, "no implicit deletion or kernel operation");
        assert.deepEqual(f.exports, f.expectedExports, "only explicit exports dispatch");
        assert.deepEqual(f.requests.filter(request => request.path === "/events/export" && request.method !== "OPTIONS")
          .map(request => ({ method: request.method, params: request.params })),
          f.expectedExports.map(params => ({ method: "GET", params })), "even malformed or fallback export requests must be explicit");
        assert.ok(f.requests.length > 0);
        for (const request of f.requests) {
          assert.ok(allowedPaths.has(request.path), request.path);
          assert.ok(["GET", "OPTIONS", "POST"].includes(request.method)); assert.equal(request.bytes, 0);
          if (request.method !== "OPTIONS") assert.equal(request.method,
            ["/events/delete", "/events/mark-read"].includes(request.path) ? "POST" : "GET");
          if (["/auth/me", "/events/unread-count", "/events/mark-read"].includes(request.path)) assert.deepEqual(request.params, {});
          if (request.method === "POST") assert.ok(["/events/delete", "/events/mark-read"].includes(request.path));
          if (request.path === "/events" && request.method === "GET") {
            assert.equal(request.params.limit, "200");
            assert.ok(Object.keys(request.params).every(key => ["limit", "category", "severity", "since_minutes", "start", "end"].includes(key)));
          }
        }
      } catch (error) { record(error); }
    }
    if (failure) throw failure;
  });
}
async function locale(f, next) {
  const menu = f.page.locator(".menu-toggle");
  if (await menu.isVisible()) await menu.click();
  await f.page.locator(".sidebar-footer select").selectOption(next);
  await f.page.waitForFunction(next => document.documentElement.lang === next, next);
  if (await menu.isVisible()) {
    await f.page.locator(".sidebar-footer select").focus(); await f.page.keyboard.press("Escape");
    assert.equal(await menu.getAttribute("aria-expanded"), "false");
  }
}
async function fitAndHit(locator, label) {
  await locator.scrollIntoViewIfNeeded();
  const r = await locator.evaluate(element => {
    const box = element.getBoundingClientRect();
    return { x: box.x, right: box.right, height: box.height, width: box.width, viewport: innerWidth,
      hit: element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)) };
  });
  assert.ok(r.width > 0 && r.height >= 44, `${label} has a 44px touch target`);
  assert.ok(r.x >= 0 && r.right <= r.viewport + 1, `${label} fits horizontally`);
  assert.equal(r.hit, true, `${label} is not obscured`);
}
async function open(f, current = "zh-CN") {
  await f.page.waitForFunction(label => [...document.querySelectorAll('[data-testid="events-danger-actions"] button')]
    .some(button => button.textContent === label && !button.disabled), text(current, "events.deleteFiltered"));
}
async function custom(f) {
  await f.select("zh-CN", "category").selectOption("platform");
  await f.select("zh-CN", "severity").selectOption("warning");
  await f.select("zh-CN", "range").selectOption("custom");
  await f.date("zh-CN", "start").fill(START); await f.date("zh-CN", "end").fill(END);
  await open(f);
  return f.page.evaluate(({ start, end }) => ({ category: "platform", severity: "warning",
    start: new Date(start).toISOString(), end: new Date(end).toISOString() }), { start: START, end: END });
}
const approve = (f, current = "zh-CN") => f.page.getByRole("dialog").getByRole("button", {
  name: text(current, "safety.confirm", { action: text(current, "events.deleteFiltered") }), exact: true,
});

eventTest("event controls partition ordinary and dangerous actions and fit five desktop mobile locales without clipping raw payloads", async f => {
  assert.equal(await f.page.locator(".events-workspace").count(), 1);
  assert.equal(await f.controls.count(), 1); assert.equal(await f.list.count(), 1);
  assert.equal(await f.actions.getByRole("button").count(), 2); assert.equal(await f.danger.getByRole("button").count(), 1);
  assert.equal(await f.actions.getByRole("button", { name: text("zh-CN", "events.deleteFiltered"), exact: true }).count(), 0);
  await custom(f);
  for (const current of ["zh-CN", "en", "es", "ja", "ko"]) {
    await f.page.setViewportSize({ width: 1440, height: 900 }); await locale(f, current);
    for (const width of [1440, 390, 320]) {
      await f.page.setViewportSize({ width, height: 900 });
      assert.ok(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${current} ${width}px`);
      for (const key of ["category", "severity", "range", "export"]) await fitAndHit(f.select(current, key), `${current} ${key}`);
      for (const key of ["start", "end"]) await fitAndHit(f.date(current, key), `${current} ${key}`);
      for (const control of [f.refresh(current), f.export(current), f.delete(current)]) await fitAndHit(control, `${current} action`);
      const a = await f.actions.boundingBox(), b = await f.danger.boundingBox();
      assert.ok(a && b && (a.x + a.width <= b.x + 1 || b.x + b.width <= a.x + 1
        || a.y + a.height <= b.y + 1 || b.y + b.height <= a.y + 1), "ordinary and dangerous groups do not overlap");
      const card = f.list.locator("article.event-card"), pre = card.locator("pre");
      assert.equal(await card.count(), 1);
      assert.deepEqual(JSON.parse(await pre.textContent()), row.payload, "raw code and every payload entry stay exact");
      const payload = await pre.evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
      assert.ok(payload.height > 0 && payload.height <= (width < 600 ? 320 : 360));
      assert.ok(payload.content > payload.height, "the complete long payload is independently scrollable");
      await pre.evaluate(element => { element.scrollTop = element.scrollHeight; });
      assert.equal(await pre.evaluate(element => element.scrollTop > 0), true);
      const bounds = await card.boundingBox(); assert.ok(bounds.x >= 0 && bounds.x + bounds.width <= width + 1);
      assert.equal(await card.getByText(row.source, { exact: false }).count(), 1);
      await f.controls.getByText(text(current, "events.markReadScope"), { exact: true }).waitFor();
    }
  }
});

eventTest("custom filters export format resize and locale changes preserve one event scope and cancelling DELETE writes nothing", async f => {
  const scope = await custom(f); await f.select("zh-CN", "export").selectOption("csv");
  const url = f.page.url(), path = await f.page.evaluate(() => window.next.router.asPath);
  const history = f.requests.filter(request => request.path === "/events" && request.method === "GET").length;
  const sockets = await f.page.evaluate(() => window.fixtureSockets.length);
  for (const [width, current] of [[1440, "en"], [960, "es"], [390, "ja"], [320, "ko"], [1440, "zh-CN"]]) {
    await f.page.setViewportSize({ width, height: 900 }); await locale(f, current);
    for (const [key, value] of [["category", "platform"], ["severity", "warning"], ["range", "custom"], ["export", "csv"]]) {
      assert.equal(await f.select(current, key).inputValue(), value);
    }
    assert.equal(await f.date(current, "start").inputValue(), START); assert.equal(await f.date(current, "end").inputValue(), END);
    assert.equal(f.page.url(), url); assert.equal(await f.page.evaluate(() => window.next.router.asPath), path);
    assert.equal(f.requests.filter(request => request.path === "/events" && request.method === "GET").length, history);
    assert.equal(await f.page.evaluate(() => window.fixtureSockets.length), sockets);
    assert.deepEqual(f.writes, []); assert.deepEqual(f.exports, []);
  }
  const lastRead = f.requests.filter(request => request.path === "/events" && request.method === "GET").at(-1);
  assert.deepEqual(lastRead.params, { ...scope, limit: "200" });
  await f.delete("zh-CN").click();
  const dialog = f.page.getByRole("dialog"); await dialog.waitFor();
  assert.equal(await approve(f).isDisabled(), true, "no phrase cannot authorize deletion");
  await dialog.getByRole("textbox").fill("delete"); assert.equal(await approve(f).isDisabled(), true);
  await dialog.getByRole("button", { name: text("zh-CN", "safety.cancel"), exact: true }).click();
  await dialog.waitFor({ state: "detached" });
  assert.equal(await f.select("zh-CN", "export").inputValue(), "csv"); assert.deepEqual(f.writes, []);
});

eventTest("explicit JSON CSV downloads and confirmed deletion dispatch exactly their own reviewed full filter scope", async f => {
  const scope = await custom(f);
  for (const format of ["json", "csv"]) {
    await f.select("zh-CN", "export").selectOption(format);
    f.expectedExports.push({ ...scope, format });
    const pending = f.page.waitForEvent("download"); await f.export("zh-CN").click();
    const download = await pending, chunks = [];
    for await (const chunk of await download.createReadStream()) chunks.push(chunk);
    assert.equal(await download.failure(), null); assert.equal(download.suggestedFilename(), `cyanrex-layout-fixture.${format}`);
    const body = Buffer.concat(chunks).toString("utf8");
    if (format === "json") assert.deepEqual(JSON.parse(body), [row]); else assert.equal(body, csv);
    assert.deepEqual(f.writes, [], "export never dispatches deletion");
    assert.deepEqual(f.exports, f.expectedExports);
  }
  f.expectedWrites = [{ path: "/events/delete", params: scope, body: null }];
  await f.delete("zh-CN").click();
  const dialog = f.page.getByRole("dialog"); await dialog.waitFor();
  assert.equal(await approve(f).isDisabled(), true);
  await dialog.getByRole("textbox").fill("DELETE"); await approve(f).click();
  await dialog.waitFor({ state: "detached" }); await open(f);
  assert.deepEqual(f.writes, f.expectedWrites); assert.deepEqual(f.exports, f.expectedExports);
  assert.equal(await f.select("zh-CN", "export").inputValue(), "csv");
  assert.deepEqual(f.requests.filter(request => request.path === "/events" && request.method === "GET").at(-1).params,
    { ...scope, limit: "200" }, "post-deletion history refresh retains the filter without export format");
});

eventTest("read acknowledgement failure stays explicit across layout changes and invalid or closed streams retain the original action guards", async f => {
  await f.page.getByRole("button", { name: text("zh-CN", "events.retryMarkRead"), exact: true }).waitFor();
  const acknowledgements = f.requests.filter(request => request.path === "/events/mark-read" && request.method === "POST").length;
  const url = f.page.url();
  for (const [width, current] of [[390, "zh-CN"], [320, "en"], [390, "es"], [320, "ja"], [390, "ko"]]) {
    await f.page.setViewportSize({ width, height: 900 }); await locale(f, current);
    await f.controls.getByText(text(current, "events.markReadFailed"), { exact: true }).waitFor();
    await fitAndHit(f.controls.getByRole("button", { name: text(current, "events.retryMarkRead"), exact: true }), `${current} read acknowledgement retry`);
    assert.equal(f.requests.filter(request => request.path === "/events/mark-read" && request.method === "POST").length, acknowledgements);
    assert.equal(f.page.url(), url);
    assert.doesNotMatch(await f.controls.innerText(), /\b(?:events|safety|layout)\.[A-Za-z][\w.]*/);
  }
  f.state.markReadStatus = 200;
  const response = f.page.waitForResponse(response => new URL(response.url()).pathname === "/events/mark-read"
    && response.request().method() === "POST");
  await f.controls.getByRole("button", { name: text("ko", "events.retryMarkRead"), exact: true }).click();
  assert.equal((await response).status(), 200);
  await f.controls.getByText(text("ko", "events.markReadPending"), { exact: true }).waitFor({ state: "detached" });
  assert.equal(await f.controls.getByText(text("ko", "events.markReadFailed"), { exact: true }).count(), 0);
  assert.equal(f.requests.filter(request => request.path === "/events/mark-read" && request.method === "POST").length, acknowledgements + 1);
  await locale(f, "zh-CN");
  await f.select("zh-CN", "range").selectOption("custom");
  await f.date("zh-CN", "start").fill("2000-01-05T00:00"); await f.date("zh-CN", "end").fill(START);
  await f.controls.getByText(text("zh-CN", "safety.invalidRange"), { exact: true }).waitFor();
  for (const control of [f.refresh("zh-CN"), f.export("zh-CN"), f.delete("zh-CN")]) assert.equal(await control.isDisabled(), true);
  await f.select("zh-CN", "range").selectOption("all"); await open(f);
  await f.page.evaluate(() => {
    const socket = window.fixtureSockets.filter(socket => !socket.closed).at(-1);
    if (!socket) throw new Error("the current original event stream must exist");
    socket.closed = true; socket.onclose({ code: 1008 });
  });
  await f.controls.getByRole("button", { name: text("zh-CN", "events.retryStream"), exact: true }).waitFor();
  assert.equal(await f.delete("zh-CN").isDisabled(), true);
  assert.equal(await f.refresh("zh-CN").isEnabled(), true); assert.equal(await f.export("zh-CN").isEnabled(), true);
  assert.equal(await f.list.locator("article.event-card").count(), 1, "terminal closure is not deletion or an empty-history claim");
  await f.controls.getByText(text("zh-CN", "events.streamGap"), { exact: true }).waitFor();
  assert.equal(f.page.url(), url); assert.deepEqual(f.writes, []); assert.deepEqual(f.exports, []);
}, { ackFailure: true });
