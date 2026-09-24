import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const stub = fileURLToPath(new URL("./fixtures/eventPageStubs.mjs", import.meta.url));
  bundle = await buildFixture(new URL("./fixtures/settingsPage.mjs", import.meta.url), {
    "next/router$": stub, "next/link$": stub, "next/head$": stub,
    [fileURLToPath(new URL("../src/utils/pageState.ts", import.meta.url))]: stub,
  });
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const events = (max_records = 500) => ({ max_records, overflow_policy: "drop_oldest" });
const compiler = (resident = false) => ({ resident, strategy: resident ? "resident_cache" : "on_demand" });
const saved = settings => ({ ok: true, settings });

async function run(callback) {
  const f = await setupFixture(browser, bundle);
  f.page.setDefaultTimeout(3000);
  f.act = callback => f.page.evaluate(callback);
  f.ids = async (path, method = "GET") => (await f.requests()).flatMap((r, i) => new URL(r.url).pathname === `/settings/${path}` && r.method === method ? [i] : []);
  f.latest = async (path, method = "GET") => (await f.ids(path, method)).at(-1);
  f.waitReads = (path, count) => f.page.waitForFunction(([path, count]) =>
    fixture.requests.filter(r => new URL(r.url).pathname === `/settings/${path}` && (r.init.method || "GET") === "GET").length >= count,
    [path, count], { polling: 10 });
  f.waitReady = () => f.page.locator('input[type="number"]:enabled').waitFor();
  const respond = f.respond;
  f.respond = async (...args) => { await respond(...args); await f.advance(1); };
  f.mount = async () => { await f.act(() => fixture.render()); await f.advance(1); };
  f.ready = async () => { await f.mount(); await f.respond(await f.latest("events"), events()); await f.respond(await f.latest("compiler"), compiler()); await f.waitReady(); };
  f.button = name => f.page.getByRole("button", { name, exact: true });
  f.approve = async () => { await f.button("Save Settings").click(); await f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ }).click(); };
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}

test("failed settings reads cannot enable saving cached defaults", () => run(async f => {
  await f.mount();
  await f.respond(await f.latest("events"), { ok: false }, 503);
  await f.respond(await f.latest("compiler"), compiler());
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  assert.equal(await f.page.getByRole("spinbutton").isDisabled(), true);
}));

test("Strict Mode obsolete settings reads cannot overwrite the current load", () => run(async f => {
  await f.ready();
  const oldEvents = (await f.ids("events"))[0], oldCompiler = (await f.ids("compiler"))[0];
  await f.respond(oldEvents, events(800)); await f.respond(oldCompiler, compiler(true));
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "500");
  assert.equal((await f.requests())[oldEvents].aborted, true);
}));

test("unconfirmed event settings cannot dispatch the compiler save", () => run(async f => {
  await f.ready(); await f.approve();
  await f.respond(await f.latest("events", "POST"), { ok: "true", settings: events() });
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.ids("compiler", "POST")).length, 0);
  assert.match(await f.page.getByRole("dialog").textContent(), /could not be confirmed/i);
}));

test("navigation cancels a pending save and cannot dispatch a late second step", () => run(async f => {
  await f.ready(); await f.approve();
  const index = await f.latest("events", "POST");
  await f.act(() => fixture.render({ navigation: "/settings?scope=next" })); await f.advance(1);
  await f.respond(index, saved(events()));
  assert.equal((await f.ids("compiler", "POST")).length, 0);
  assert.equal((await f.requests())[index].aborted, true);
}));

test("partial settings saves require verification before a new confirmation", () => run(async f => {
  await f.ready(); await f.approve();
  await f.respond(await f.latest("events", "POST"), saved(events()));
  await f.respond(await f.latest("compiler", "POST"), { ok: false, message: "private synthetic detail" }, 503);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.match(await f.page.getByRole("dialog").textContent(), /Event settings were saved/);
  assert.doesNotMatch(await f.page.getByRole("dialog").textContent(), /private synthetic detail/);
  await f.page.keyboard.press("Escape");
  assert.equal(await f.button("Save Settings").isDisabled(), true);
}));

test("stalled initial settings requests have a bounded read wait and explicit reload", () => run(async f => {
  await f.mount(); await f.advance(10010);
  assert.equal(await f.button("Reload Settings").count(), 1);
  assert.equal(await f.button("Reload Settings").isDisabled(), false);
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  assert.ok((await f.requests()).every(r => r.aborted));
}));

test("successful saves freeze normalized values, lock editing and dispatch each stage only once", () => run(async f => {
  await f.ready();
  await f.page.getByRole("spinbutton").fill("20");
  await f.page.getByRole("combobox").filter({ has: f.page.locator('option[value="drop_new"]') }).selectOption("drop_new");
  await f.page.getByRole("checkbox").check();
  await f.button("Save Settings").click();
  const dialog = f.page.getByRole("dialog"), desired = { max_records: 50, overflow_policy: "drop_new" };
  assert.match(await dialog.textContent(), /50/);
  assert.equal(await f.page.getByRole("spinbutton", { includeHidden: true }).isDisabled(), true);
  await dialog.getByRole("button", { name: /^Confirm/ }).evaluate(el => { el.click(); el.click(); });
  assert.equal((await f.ids("events", "POST")).length, 1);
  const eventRequest = (await f.requests())[await f.latest("events", "POST")];
  assert.deepEqual(eventRequest.body, desired);
  assert.equal(eventRequest.cache, "no-store"); assert.equal(eventRequest.redirect, "error"); assert.equal(eventRequest.credentials, "include");
  await f.respond(await f.latest("events", "POST"), saved(desired));
  assert.equal((await f.ids("compiler", "POST")).length, 1);
  assert.deepEqual((await f.requests())[await f.latest("compiler", "POST")].body, { resident: true });
  await f.respond(await f.latest("compiler", "POST"), saved(compiler(true)));
  await dialog.waitFor({ state: "detached" });
  await f.page.getByRole("status").filter({ hasText: /^Settings saved$/ }).waitFor();
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "50");
  assert.equal(await f.button("Save Settings").isDisabled(), false);
  assert.equal(await f.act(() => fixture.unreadChanges), 1);
}));

test("unavailable compiler reads explicitly allow event-only saving, never a hidden compiler write", () => run(async f => {
  await f.mount();
  await f.respond(await f.latest("events"), events());
  await f.respond(await f.latest("compiler"), { resident: false, strategy: "resident_cache" });
  await f.page.getByRole("status").filter({ hasText: /Only event settings/ }).waitFor();
  assert.equal(await f.page.getByRole("checkbox").count(), 0);
  await f.approve(); await f.respond(await f.latest("events", "POST"), saved(events()));
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  await f.page.getByRole("status").filter({ hasText: /Compiler settings were not changed/ }).waitFor();
  assert.equal((await f.ids("compiler", "POST")).length, 0);
}));

test("malformed event reads stay blocked until an explicit fresh reload succeeds", () => run(async f => {
  await f.mount();
  await f.respond(await f.latest("events"), { ...events(), max_records: "500" });
  await f.respond(await f.latest("compiler"), compiler());
  await f.page.getByRole("alert").waitFor();
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  const count = (await f.ids("events")).length;
  await f.button("Reload Settings").click(); await f.waitReads("events", count + 1);
  assert.equal((await f.ids("events", "POST")).length, 0);
  await f.respond(await f.latest("events"), events(900));
  await f.respond(await f.latest("compiler"), compiler(true));
  await f.waitReady();
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "900");
  assert.equal(await f.button("Save Settings").isDisabled(), false);
}));

test("event save timeout blocks retry and ignores late acknowledgement before fresh verification", () => run(async f => {
  await f.ready(); await f.approve();
  const index = await f.latest("events", "POST");
  await f.advance(20010); await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.requests())[index].aborted, true);
  await f.page.keyboard.press("Escape");
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  await f.respond(index, saved(events()));
  assert.equal((await f.ids("compiler", "POST")).length, 0);
  assert.equal(await f.act(() => fixture.unreadChanges), 0);
  const count = (await f.ids("events")).length;
  await f.button("Reload Settings").click(); await f.waitReads("events", count + 1);
  await f.respond(await f.latest("events"), events(700)); await f.respond(await f.latest("compiler"), compiler());
  await f.waitReady();
  assert.equal((await f.ids("events", "POST")).length, 1, "reload never retries the write");
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "700");
  await f.approve();
  assert.deepEqual((await f.requests())[await f.latest("events", "POST")].body, events(700));
}));

test("compiler timeout retains partial-save warning even after metrics refresh", () => run(async f => {
  await f.ready(); await f.approve(); await f.respond(await f.latest("events", "POST"), saved(events()));
  const index = await f.latest("compiler", "POST");
  await f.advance(20010); await f.page.getByRole("dialog").getByRole("alert").waitFor();
  await f.page.keyboard.press("Escape");
  await f.button("Refresh metrics").click(); await f.advance(1);
  assert.match(await f.page.getByRole("alert").textContent(), /Event settings were saved/);
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  await f.respond(index, saved(compiler()));
  assert.equal(await f.button("Save Settings").isDisabled(), true);
  assert.equal(await f.act(() => fixture.unreadChanges), 1);
}));

test("unmount aborts saving and prevents late writes or unread notifications", () => run(async f => {
  await f.ready(); await f.approve();
  const index = await f.latest("events", "POST");
  await f.act(() => fixture.render(false)); await f.advance(1);
  await f.respond(index, saved(events()));
  assert.equal((await f.requests())[index].aborted, true);
  assert.equal((await f.ids("compiler", "POST")).length, 0);
  assert.equal(await f.act(() => fixture.unreadChanges), 0);
}));

test("cancelling a review or navigating before confirmation never writes", () => run(async f => {
  await f.ready(); await f.button("Save Settings").click(); await f.page.keyboard.press("Escape");
  assert.equal((await f.ids("events", "POST")).length, 0);
  await f.button("Save Settings").click();
  await f.act(() => fixture.render({ navigation: "/settings?scope=review-cancel" })); await f.advance(1);
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.equal(await f.page.getByRole("dialog").count(), 0);
  assert.equal((await f.ids("events", "POST")).length, 0);
}));

test("blank and fractional drafts cannot be confirmed; discarding a draft needs explicit approval", () => run(async f => {
  await f.ready();
  for (const value of ["", "50.5"]) {
    await f.page.getByRole("spinbutton").fill(value); await f.button("Save Settings").click();
    assert.equal(await f.page.getByRole("dialog").count(), 0);
    assert.match(await f.page.getByRole("alert").textContent(), /whole-number/);
  }
  const reads = (await f.ids("events")).length;
  await f.button("Reload Settings").click();
  await f.page.getByRole("dialog").waitFor();
  assert.equal((await f.ids("events")).length, reads);
  await f.page.keyboard.press("Escape");
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "50.5");
  await f.button("Reload Settings").click();
  await f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ }).click(); await f.advance(1);
  await f.respond(await f.latest("events"), events()); await f.respond(await f.latest("compiler"), compiler());
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "500");
  assert.equal((await f.ids("events", "POST")).length, 0);
}));

test("locale changes preserve the draft and never reload or mutate settings", () => run(async f => {
  await f.ready(); await f.page.getByRole("spinbutton").fill("717");
  const count = (await f.requests()).length;
  for (const [locale, reload] of [["zh-CN", "重新读取设置"], ["es", "Recargar configuración"], ["ja", "設定を再読み込み"], ["en", "Reload Settings"]]) {
    await f.page.evaluate(locale => fixture.setLocale(locale), locale); await f.advance(1);
    assert.equal(await f.button(reload).count(), 1);
    assert.equal(await f.page.getByRole("spinbutton").inputValue(), "717");
  }
  assert.equal((await f.requests()).length, count);
}));

test("new navigation owns loading state and rejects all earlier read results", () => run(async f => {
  await f.mount();
  assert.equal(await f.page.getByRole("spinbutton").isDisabled(), true);
  const oldEvents = await f.latest("events"), oldCompiler = await f.latest("compiler");
  await f.act(() => fixture.render({ navigation: "/settings?scope=fresh" })); await f.advance(1);
  await f.respond(await f.latest("events"), events(900)); await f.respond(await f.latest("compiler"), compiler());
  await f.respond(oldEvents, events(600)); await f.respond(oldCompiler, compiler(true));
  assert.equal(await f.page.getByRole("spinbutton").inputValue(), "900");
  assert.equal(await f.page.getByRole("checkbox").isChecked(), false);
  assert.equal((await f.requests())[oldEvents].aborted, true);
}));

test("a mismatched compiler acknowledgement cannot claim complete success", () => run(async f => {
  await f.ready(); await f.approve();
  await f.respond(await f.latest("events", "POST"), saved(events()));
  await f.respond(await f.latest("compiler", "POST"), saved(compiler(true)));
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.match(await f.page.getByRole("dialog").textContent(), /Event settings were saved/);
  await f.page.keyboard.press("Escape");
  assert.equal(await f.button("Save Settings").isDisabled(), true);
}));
