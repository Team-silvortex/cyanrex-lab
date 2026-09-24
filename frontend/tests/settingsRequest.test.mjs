import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { SETTINGS_READ_TIMEOUT_MS, SETTINGS_SAVE_TIMEOUT_MS, normalizeEventDraft,
  parseEventSettings, parseCompilerSettings, parseEventSettingsSaved, parseCompilerSettingsSaved,
  requestSettings } from "../src/features/settings/settingsRequest.ts";

const events = { max_records: 500, overflow_policy: "drop_oldest" };
const compiler = { resident: false, strategy: "on_demand" };
const url = "https://engine.invalid/settings/events";
const signal = () => new AbortController().signal;
const json = value => Response.json(value);

test("event settings accept only integral limits and exact policies", () => {
  for (const max_records of [50, 500, 50000]) assert.equal(parseEventSettings({ ...events, max_records }).max_records, max_records);
  for (const max_records of [null, true, "500", 49, 50001, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])
    assert.throws(() => parseEventSettings({ ...events, max_records }));
  for (const value of [null, [], {}, { ...events, overflow_policy: ["drop_oldest"] }, { ...events, overflow_policy: "other" }])
    assert.throws(() => parseEventSettings(value));
});

test("reviewed drafts clamp valid integers but never coerce invalid inputs", () => {
  assert.deepEqual(normalizeEventDraft({ ...events, max_records: 20 }), { ...events, max_records: 50 });
  assert.deepEqual(normalizeEventDraft({ ...events, max_records: 90000 }), { ...events, max_records: 50000 });
  for (const max_records of [NaN, 50.1, "500", Infinity]) assert.throws(() => normalizeEventDraft({ ...events, max_records }));
});

test("compiler reads require a boolean and matching strategy", () => {
  assert.deepEqual(parseCompilerSettings(compiler), compiler);
  assert.deepEqual(parseCompilerSettings({ resident: true, strategy: "resident_cache" }), { resident: true, strategy: "resident_cache" });
  for (const value of [null, {}, { resident: "false", strategy: "on_demand" }, { ...compiler, strategy: "resident_cache" }])
    assert.throws(() => parseCompilerSettings(value));
});

test("write acknowledgements must strictly confirm the reviewed event and compiler values", () => {
  assert.deepEqual(parseEventSettingsSaved({ ok: true, settings: events }, events), events);
  assert.deepEqual(parseCompilerSettingsSaved({ ok: true, settings: compiler }, false), compiler);
  for (const value of [null, {}, { ok: "true", settings: events }, { ok: true }, { ok: true, settings: { ...events, max_records: 800 } }])
    assert.throws(() => parseEventSettingsSaved(value, events));
  for (const value of [{ ok: 1, settings: compiler }, { ok: true }, { ok: true, settings: { resident: true, strategy: "resident_cache" } }])
    assert.throws(() => parseCompilerSettingsSaved(value, false));
});

test("settings requests are private credentialed reads/writes without authority overrides or retries", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", (_url, init) => { calls.push(init); return json(events); });
  assert.deepEqual(await requestSettings(url, signal(), parseEventSettings), events);
  assert.deepEqual(await requestSettings(url, signal(), parseEventSettings, events), events);
  assert.equal(calls.length, 2); assert.equal(calls[0].method, "GET"); assert.equal(calls[0].body, undefined);
  assert.equal(calls[1].method, "POST"); assert.deepEqual(JSON.parse(calls[1].body), events);
  assert.deepEqual(calls[1].headers, { "Content-Type": "application/json" });
  for (const call of calls) { assert.equal(call.credentials, "include"); assert.equal(call.cache, "no-store"); assert.equal(call.redirect, "error"); }
});

test("pre-cancelled settings requests never dispatch", async t => {
  const fetch = t.mock.method(globalThis, "fetch", () => { throw new Error("must not dispatch"); });
  const parent = new AbortController(); parent.abort();
  await assert.rejects(requestSettings(url, parent.signal, parseEventSettings, events), { name: "AbortError" });
  assert.equal(fetch.mock.callCount(), 0);
});

test("read deadlines end waiting even if transport ignores abort; late headers never decode", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let release, request, decoded = 0;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return new Promise(resolve => { release = resolve; }); });
  const pending = requestSettings(url, signal(), () => { decoded++; });
  t.mock.timers.tick(SETTINGS_READ_TIMEOUT_MS - 1); assert.equal(request.signal.aborted, false);
  t.mock.timers.tick(1); await assert.rejects(pending, { name: "TimeoutError" });
  release(json(events)); await Promise.resolve(); await Promise.resolve();
  assert.equal(decoded, 0); assert.equal(request.signal.aborted, true);
});

test("write deadline also bounds an abort-ignoring JSON body and removes cancellation linkage", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let release, request, decoded = 0;
  const parent = new AbortController(), removed = t.mock.method(parent.signal, "removeEventListener");
  t.mock.method(globalThis, "fetch", (_url, init) => {
    request = init; return { ok: true, headers: new Headers({ "content-type": "application/json" }), json: () => new Promise(resolve => { release = resolve; }) };
  });
  const pending = requestSettings(url, parent.signal, () => { decoded++; }, events);
  await Promise.resolve(); t.mock.timers.tick(SETTINGS_SAVE_TIMEOUT_MS - 1); assert.equal(request.signal.aborted, false);
  t.mock.timers.tick(1); await assert.rejects(pending, { name: "TimeoutError" });
  release(events); await Promise.resolve(); await Promise.resolve();
  assert.equal(decoded, 0); assert.equal(removed.mock.callCount(), 1); assert.equal(parent.signal.aborted, false);
});

test("navigation cancellation ends a settings write before late transport completion", async t => {
  let release, request;
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return new Promise(resolve => { release = resolve; }); });
  const parent = new AbortController(), pending = requestSettings(url, parent.signal, parseEventSettings, events);
  parent.abort(); await assert.rejects(pending, { name: "AbortError" }); assert.equal(request.signal.aborted, true);
  release(json(events)); await Promise.resolve();
});

test("completed reads release both timer and parent linkage", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let request;
  const parent = new AbortController();
  t.mock.method(globalThis, "fetch", (_url, init) => { request = init; return json(events); });
  await requestSettings(url, parent.signal, parseEventSettings);
  parent.abort(); t.mock.timers.tick(SETTINGS_SAVE_TIMEOUT_MS); assert.equal(request.signal.aborted, false);
});

test("HTTP, network, JSON and MIME failures cannot retry or confirm a write", async t => {
  let count = 0;
  for (const status of [400, 401, 403, 408, 413, 429, 500, 503]) {
    t.mock.method(globalThis, "fetch", () => { count++; return Response.json({ message: "synthetic detail" }, { status }); });
    await assert.rejects(requestSettings(url, signal(), parseEventSettings, events), /Settings request failed/);
  }
  t.mock.method(globalThis, "fetch", () => { count++; throw new TypeError("offline"); });
  await assert.rejects(requestSettings(url, signal(), parseEventSettings, events), /offline/);
  t.mock.method(globalThis, "fetch", () => { count++; return new Response("{broken", { headers: { "content-type": "application/json" } }); });
  await assert.rejects(requestSettings(url, signal(), parseEventSettings, events), SyntaxError);
  t.mock.method(globalThis, "fetch", () => { count++; return new Response(JSON.stringify(events), { headers: { "content-type": "text/html" } }); });
  await assert.rejects(requestSettings(url, signal(), parseEventSettings, events), /Settings request failed/);
  assert.equal(count, 11);
});

test("all four locales explain unavailable, unconfirmed and partial settings updates", () => {
  for (const locale of ["en", "zh_cn", "es", "ja"]) {
    const source = readFileSync(new URL(`../src/i18n/locales/${locale}.ts`, import.meta.url), "utf8");
    for (const key of ["loadFailed", "compilerUnavailable", "eventsOnlySaved", "saveUnconfirmed",
      "partialSave", "verifyBeforeSave", "invalidDraft", "reload", "reloadHint"]) assert.ok(source.includes(`${key}: "`), `${locale}.${key}`);
  }
});
