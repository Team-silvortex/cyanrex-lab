import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const stub = fileURLToPath(new URL("./fixtures/eventPageStubs.mjs", import.meta.url));
  bundle = await buildFixture(new URL("./fixtures/aiAgentSettings.mjs", import.meta.url), {
    "next/router$": stub, "next/link$": stub, "next/head$": stub,
    [fileURLToPath(new URL("../src/utils/pageState.ts", import.meta.url))]: stub,
  });
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const profile = (extra = {}) => ({ id: "primary", name: "Team assistant", protocol: "openai_responses",
  base_url: "https://provider.invalid/v1", model: "chosen-by-operator", credential_ref: "TEAM_AI_KEY", enabled: true, ...extra });
const settings = (extra = {}) => ({ revision: 3, default_profile_id: "primary", profiles: [profile()], ...extra });
const saved = body => ({ ok: true, settings: { revision: body.expected_revision + 1,
  default_profile_id: body.default_profile_id, profiles: body.profiles } });

async function run(callback) {
  const f = await setupFixture(browser, bundle); f.page.setDefaultTimeout(3000);
  f.mount = async (options = {}) => { await f.page.evaluate(options => fixture.render(options), options); await f.advance(1); };
  f.ids = async (method = "GET") => (await f.requests()).flatMap((r, i) => r.method === method ? [i] : []);
  f.latest = async method => (await f.ids(method)).at(-1);
  const respond = f.respond; f.respond = async (...args) => { await respond(...args); await f.advance(1); };
  f.button = name => f.page.getByRole("button", { name, exact: true });
  f.input = name => f.page.getByRole("textbox", { name, exact: true });
  f.confirm = () => f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ });
  f.closeDialog = async () => { await f.page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click(); await f.page.getByRole("dialog").waitFor({ state: "detached" }); };
  f.waitReads = count => f.page.waitForFunction(count => fixture.requests.filter(r => (r.init.method || "GET") === "GET").length >= count, count, { polling: 10 });
  f.reloadDraft = async () => {
    const count = (await f.ids()).length;
    await f.button("Reload AI configurations").click(); await f.confirm().click();
    await f.waitReads(count + 1); await f.page.getByRole("dialog").waitFor({ state: "detached" });
  };
  f.ready = async (value = settings()) => { await f.mount(); await f.respond(await f.latest(), value); await f.button("Add AI configuration").waitFor(); };
  f.save = async () => { await f.button("Save AI configurations").click(); await f.confirm().click(); await f.advance(1); return f.latest("POST"); };
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}

test("invalid initial read stays blocked and does not affect neighboring Settings", () => run(async f => {
  await f.mount({ mode: "page" }); await f.respond(await f.latest(), settings({ profiles: [profile({ enabled: false })] }));
  assert.equal(await f.button("Add AI configuration").isDisabled(), true);
  assert.equal(await f.button("Save AI configurations").isDisabled(), true);
  assert.equal(await f.button("Save Settings").isDisabled(), false);
  const count = (await f.ids()).length; await f.advance(20010); assert.equal((await f.ids()).length, count);
}));

test("Strict Mode discards old reads; initial timeout releases explicit reload", () => run(async f => {
  await f.mount(); const old = (await f.ids())[0]; await f.advance(10010);
  assert.equal(await f.button("Reload AI configurations").isDisabled(), false);
  await f.button("Reload AI configurations").click(); await f.advance(1);
  await f.respond(await f.latest(), settings()); await f.respond(old, settings({ profiles: [profile({ name: "obsolete" })] }));
  assert.equal(await f.input("Display name").inputValue(), "Team assistant");
  assert.equal((await f.requests())[old].aborted, true); assert.equal((await f.ids("POST")).length, 0);
}));

test("new profiles start disabled with no model or endpoint and all protocols are configurable", () => run(async f => {
  await f.ready({ revision: 0, default_profile_id: null, profiles: [] }); await f.button("Add AI configuration").click();
  assert.equal(await f.input("Model").inputValue(), ""); assert.equal(await f.input("Base URL").inputValue(), "");
  assert.equal(await f.page.getByRole("checkbox", { name: "Enabled" }).isChecked(), false);
  await f.button("Save AI configurations").click(); assert.equal(await f.page.getByRole("dialog").count(), 0);
  for (const protocol of ["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "custom"])
    await f.page.getByRole("combobox", { name: "Provider protocol" }).selectOption(protocol);
  assert.equal(await f.button("Use protocol example URL").count(), 0);
  await f.page.getByRole("combobox", { name: "Provider protocol" }).selectOption("anthropic_messages");
  await f.button("Use protocol example URL").click(); assert.equal(await f.input("Base URL").inputValue(), "https://api.anthropic.com");
  assert.equal((await f.ids("POST")).length, 0);
}));

test("confirmed save freezes Engine, revision and exact metadata, and only dispatches once", () => run(async f => {
  await f.ready(); await f.input("Display name").fill("Reviewed name"); await f.button("Save AI configurations").click();
  const dialog = f.page.getByRole("dialog"); assert.match(await dialog.textContent(), /https:\/\/engine-a.invalid/);
  assert.match(await dialog.textContent(), /Reviewed name/); assert.equal((await f.ids("POST")).length, 0);
  assert.equal(await f.input("Display name").isDisabled(), true);
  await f.confirm().evaluate(element => { element.click(); element.click(); }); await f.advance(1);
  const ids = await f.ids("POST"); assert.equal(ids.length, 1); const request = (await f.requests())[ids[0]];
  assert.deepEqual(request.body, { expected_revision: 3, default_profile_id: "primary", profiles: [profile({ name: "Reviewed name" })] });
  assert.equal(request.credentials, "include"); assert.equal(request.redirect, "error"); assert.equal(request.cache, "no-store");
  await f.respond(ids[0], saved(request.body)); await dialog.waitFor({ state: "detached" });
  await f.page.getByRole("status").filter({ hasText: /saved and confirmed/ }).waitFor();
  assert.equal(await f.button("Save AI configurations").isDisabled(), true); assert.equal(await f.input("Display name").isDisabled(), false);
  assert.ok((await f.requests()).every(request => new URL(request.url).origin === "https://engine-a.invalid"));
}));

test("conflict retains a read-only draft; failed reload cannot discard it or retry saving", () => run(async f => {
  await f.ready(); await f.input("Display name").fill("Unsaved change"); const post = await f.save();
  await f.respond(post, { message: "private storage detail" }, 409);
  await f.page.getByRole("dialog").getByRole("alert").waitFor(); assert.match(await f.page.getByRole("dialog").textContent(), /saved first/);
  assert.doesNotMatch(await f.page.locator("body").textContent(), /private storage detail/); await f.closeDialog();
  assert.equal(await f.input("Display name").inputValue(), "Unsaved change"); assert.equal(await f.input("Display name").isDisabled(), true);
  await f.advance(30010); assert.equal((await f.ids("POST")).length, 1);
  await f.reloadDraft();
  await f.respond(await f.latest(), {}, 503); await f.page.getByRole("alert").filter({ hasText: /could not be verified/ }).waitFor();
  assert.equal(await f.input("Display name").inputValue(), "Unsaved change");
  assert.equal(await f.button("Save AI configurations").isDisabled(), true);
  await f.reloadDraft();
  await f.respond(await f.latest(), settings({ revision: 4, profiles: [profile({ name: "Server change" })] }));
  await f.page.waitForFunction(() => document.querySelector('input[aria-label="Display name"]').value === "Server change");
  assert.equal(await f.input("Display name").inputValue(), "Server change"); assert.equal(await f.input("Display name").isDisabled(), false);
  assert.equal((await f.ids("POST")).length, 1);
}));

test("stalled write body times out, preserves draft and ignores a late matching acknowledgement", () => run(async f => {
  await f.ready(); await f.input("Model").fill("my-other-model"); const post = await f.save(), body = (await f.requests())[post].body;
  await f.page.evaluate(index => fixture.requests[index].resolve({ ok: true, status: 200,
    headers: new Headers({ "content-type": "application/json" }), json: () => new Promise(resolve => { fixture.releaseBody = resolve; }) }), post);
  await f.advance(20010); await f.page.getByRole("dialog").getByRole("alert").waitFor(); await f.closeDialog();
  assert.equal((await f.requests())[post].aborted, true);
  await f.page.evaluate(value => fixture.releaseBody(value), saved(body)); await f.advance(1);
  assert.equal(await f.input("Model").inputValue(), "my-other-model"); assert.equal(await f.input("Model").isDisabled(), true);
  assert.equal((await f.ids("POST")).length, 1); assert.doesNotMatch(await f.page.locator("body").textContent(), /saved and confirmed/);
}));

test("mismatched successful acknowledgement becomes uncertain rather than silently adopting different settings", () => run(async f => {
  await f.ready(); await f.input("Model").fill("desired-model"); const post = await f.save();
  await f.respond(post, { ok: true, settings: settings({ revision: 4 }) });
  await f.page.getByRole("dialog").getByRole("alert").waitFor(); await f.closeDialog();
  assert.equal(await f.input("Model").inputValue(), "desired-model"); assert.equal(await f.button("Save AI configurations").isDisabled(), true);
}));

test("Engine switch aborts the pending write, clears its dialog and ignores its late success", () => run(async f => {
  await f.ready(); await f.input("Model").fill("engine-a-model"); const post = await f.save(), body = (await f.requests())[post].body;
  await f.mount({ engineUrl: "https://engine-b.invalid" }); assert.equal((await f.requests())[post].aborted, true);
  assert.equal(await f.page.getByRole("dialog").count(), 0);
  await f.respond(await f.latest(), settings({ profiles: [profile({ model: "engine-b-model" })] }));
  await f.respond(post, saved(body)); assert.equal(await f.input("Model").inputValue(), "engine-b-model");
  assert.equal((await f.ids("POST")).length, 1); assert.equal(await f.button("Save AI configurations").isDisabled(), true);
}));

test("navigation cancels confirmation before dispatch and cannot retain obsolete read results", () => run(async f => {
  await f.ready(); await f.input("Model").fill("unsaved"); await f.button("Save AI configurations").click();
  const count = (await f.ids()).length; await f.mount({ navigation: "/settings?scope=next" });
  await f.page.getByRole("dialog").waitFor({ state: "detached" }); await f.waitReads(count + 1);
  await f.respond(await f.latest(), settings({ profiles: [profile({ model: "fresh" })] }));
  assert.equal(await f.input("Model").inputValue(), "fresh"); assert.equal((await f.ids("POST")).length, 0);
}));

test("unmount ends a pending write and observes late rejected transport without retry", () => run(async f => {
  await f.ready(); await f.input("Model").fill("new-model"); const post = await f.save(); await f.mount(false);
  assert.equal((await f.requests())[post].aborted, true); const count = (await f.requests()).length;
  await f.reject(post); await f.advance(30010); assert.equal((await f.requests()).length, count);
}));

test("navigation during a submitted save aborts waiting and cannot adopt its late headers", () => run(async f => {
  await f.ready(); await f.input("Model").fill("old-navigation-model"); const post = await f.save(), body = (await f.requests())[post].body;
  const count = (await f.ids()).length; await f.mount({ navigation: "/settings?scope=after-write" });
  await f.waitReads(count + 1); await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.equal((await f.requests())[post].aborted, true);
  await f.respond(await f.latest(), settings({ revision: 7, profiles: [profile({ model: "new-navigation-model" })] }));
  await f.respond(post, saved(body)); assert.equal(await f.input("Model").inputValue(), "new-navigation-model");
  assert.equal(await f.button("Save AI configurations").isDisabled(), true); assert.equal((await f.ids("POST")).length, 1);
}));

test("disabling or removing a default clears it in the draft and removal only persists after confirmation", () => run(async f => {
  await f.ready(); await f.page.getByRole("checkbox", { name: "Enabled" }).uncheck();
  assert.equal(await f.page.getByRole("combobox", { name: "Default configuration" }).inputValue(), "");
  await f.page.getByRole("checkbox", { name: "Enabled" }).check();
  await f.page.getByRole("combobox", { name: "Default configuration" }).selectOption("primary");
  await f.button("Remove from draft").click(); assert.equal((await f.ids("POST")).length, 0);
  const post = await f.save(); assert.deepEqual((await f.requests())[post].body, { expected_revision: 3, default_profile_id: null, profiles: [] });
  await f.respond(post, saved((await f.requests())[post].body)); await f.page.getByRole("dialog").waitFor({ state: "detached" });
}));

test("reload cancellation and locale changes preserve the draft without network or storage writes", () => run(async f => {
  await f.ready(); await f.input("Display name").fill("Keep this draft"); const count = (await f.requests()).length;
  await f.button("Reload AI configurations").click(); await f.page.keyboard.press("Escape"); await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.equal(await f.input("Display name").inputValue(), "Keep this draft");
  for (const [locale, label] of [["zh-CN", "重新读取 AI 配置"], ["es", "Recargar configuraciones AI"], ["ja", "AI 設定を再読み込み"], ["en", "Reload AI configurations"]]) {
    await f.page.evaluate(locale => fixture.setLocale(locale), locale); await f.advance(1); await f.button(label).waitFor();
  }
  assert.equal(await f.input("Display name").inputValue(), "Keep this draft"); assert.equal((await f.requests()).length, count);
  assert.deepEqual(await f.page.evaluate(() => fixture.saved), {});
}));

test("raw credential values and unsafe provider endpoints cannot reach confirmation", () => run(async f => {
  await f.ready(); await f.input("Credential reference (optional)").fill("sk-not-a-real-key");
  await f.button("Save AI configurations").click(); assert.equal(await f.page.getByRole("dialog").count(), 0);
  await f.input("Credential reference (optional)").fill("TEAM_AI_KEY"); await f.input("Base URL").fill("http://192.168.1.5/v1");
  await f.button("Save AI configurations").click(); assert.equal(await f.page.getByRole("dialog").count(), 0);
  assert.equal((await f.ids("POST")).length, 0);
}));

test("profile bound disables addition without creating an extra draft entry", () => run(async f => {
  await f.ready(settings({ default_profile_id: null, profiles: Array.from({ length: 16 }, (_, i) => profile({ id: `agent-${i}` })) }));
  assert.equal(await f.button("Add AI configuration").isDisabled(), true); assert.equal(await f.page.getByRole("group").count(), 16);
  assert.equal((await f.ids("POST")).length, 0);
}));
