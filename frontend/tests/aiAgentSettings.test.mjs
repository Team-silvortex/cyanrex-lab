import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
import { aiAgentMessages } from "../src/i18n/locales/aiAgents.ts";

const moduleUrl = new URL("../src/features/aiAgents/aiAgentSettings.ts", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, nextResolve) {
  if (context.parentURL === moduleUrl && specifier === "../../transport/privateRequest")
    return nextResolve(new URL("../src/transport/privateRequest.ts", import.meta.url).href, context);
  return nextResolve(specifier, context);
} });
const { AI_AGENT_PROTOCOLS, AI_AGENT_READ_TIMEOUT_MS, AI_AGENT_SAVE_TIMEOUT_MS,
  parseAiAgentSettings, reviewAiAgentSettings, parseAiAgentSettingsSaved, requestAiAgentSettings } = await import(moduleUrl);
resolver.deregister();
const profile = (extra = {}) => ({ id: "primary", name: "Team assistant", protocol: "openai_responses",
  base_url: "https://provider.invalid/v1", model: "chosen-by-operator", credential_ref: "TEAM_AI_KEY", enabled: true, ...extra });
const settings = (extra = {}) => ({ revision: 3, default_profile_id: "primary", profiles: [profile()], ...extra });
const signal = () => new AbortController().signal;
const url = "https://engine.invalid/settings/ai-agents";
const saved = body => ({ ok: true, settings: { revision: body.expected_revision + 1,
  default_profile_id: body.default_profile_id, profiles: body.profiles } });

test("all five provider protocols preserve exact metadata and optional symbolic references", () => {
  for (const protocol of AI_AGENT_PROTOCOLS) {
    const value = settings({ profiles: [profile({ protocol, name: "  Multilingual 中文 🚀  ", credential_ref: null })] });
    assert.deepEqual(parseAiAgentSettings(value), value);
    assert.notEqual(parseAiAgentSettings(value).profiles[0], value.profiles[0]);
  }
  assert.deepEqual(parseAiAgentSettings({ revision: 0, default_profile_id: null, profiles: [] }),
    { revision: 0, default_profile_id: null, profiles: [] });
});

test("reads reject malformed settings, duplicate IDs, disabled defaults and credential-shaped fields", () => {
  for (const value of [null, [], {}, settings({ revision: -1 }), settings({ revision: 1.5 }),
    settings({ revision: 2 ** 32 }), settings({ revision: "3" }), settings({ default_profile_id: "missing" }),
    settings({ profiles: [profile({ enabled: false })] }), settings({ profiles: [profile(), profile()] }),
    settings({ profiles: Array.from({ length: 17 }, (_, i) => profile({ id: `p${i}` })) }),
    settings({ api_key: "not-a-real-key" }), settings({ profiles: [profile({ api_key: "not-a-real-key" })] })])
    assert.throws(() => parseAiAgentSettings(value));
});

test("profile validation enforces symbols, Unicode scalar limits, booleans and bounded nonblank text", () => {
  for (const extra of [{ id: "Upper" }, { id: "a".repeat(65) }, { id: "1bad" }, { name: " \t " },
    { model: "" }, { model: "m".repeat(129) }, { name: "x\u0085" }, { model: "x\n" },
    { credential_ref: "env:KEY" }, { credential_ref: "sk-fake-secret" }, { credential_ref: "" },
    { credential_ref: "A".repeat(65) }, { enabled: "true" }, { protocol: "unknown" }])
    assert.throws(() => parseAiAgentSettings(settings({ profiles: [profile(extra)] })));
  assert.equal(parseAiAgentSettings(settings({ profiles: [profile({ model: "🚀".repeat(128) })] })).profiles[0].model.length, 256);
});

test("endpoints allow HTTPS and exact local HTTP only, without inline authentication or query secrets", () => {
  for (const base_url of ["https://provider.invalid/v1", "https://provider.invalid/path@name", "https://192.168.1.4:8443/api", "http://localhost:8000/v1", "http://127.0.0.1:8000", "http://[::1]:8000/v1"])
    assert.equal(parseAiAgentSettings(settings({ profiles: [profile({ base_url })] })).profiles[0].base_url, base_url);
  for (const base_url of ["http://192.168.1.4/v1", "http://127.1/v1", "http://2130706433/v1", "http://0177.0.0.1/v1",
    "http://provider.invalid", "http://localhost.evil.invalid", "file:///tmp/x",
    "/v1", "https://provider.invalid:/v1", "https://[::1]:/v1", "http://localhost:/v1",
    "https://user:secret@provider.invalid", "https://provider.invalid?key=secret", "https://provider.invalid/#fragment",
    "https://provider.invalid/?", "https://*.invalid/v1", "https://provider.invalid/has space", "https://provider.invalid/\uFEFF", " https://provider.invalid", "https://provider.invalid/" + "x".repeat(2048)])
    assert.throws(() => parseAiAgentSettings(settings({ profiles: [profile({ base_url })] })), base_url);
});

test("review creates independent exact snapshots and refuses exhausted revision or oversized JSON", () => {
  const value = settings(), body = reviewAiAgentSettings(value);
  assert.deepEqual(body, { expected_revision: 3, default_profile_id: "primary", profiles: value.profiles });
  value.profiles[0].name = "changed after review"; assert.equal(body.profiles[0].name, "Team assistant");
  assert.throws(() => reviewAiAgentSettings(settings({ revision: 2 ** 32 - 1 })));
  const large = settings({ default_profile_id: null, profiles: Array.from({ length: 16 }, (_, i) => profile({
    id: `p${i}`, base_url: "https://provider.invalid/" + "x".repeat(1950), name: "🚀".repeat(128), model: "🚀".repeat(128) })) });
  assert.throws(() => reviewAiAgentSettings(large));
});

test("write acknowledgement must confirm exact reviewed data and the next revision", () => {
  const body = reviewAiAgentSettings(settings());
  assert.deepEqual(parseAiAgentSettingsSaved(saved(body), body), saved(body).settings);
  for (const value of [{ ok: "true", settings: saved(body).settings }, { ...saved(body), extra: true },
    { ok: true, settings: settings() }, { ok: true, settings: settings({ revision: 5 }) },
    { ok: true, settings: settings({ revision: 4, profiles: [profile({ name: "different" })] }) }])
    assert.throws(() => parseAiAgentSettingsSaved(value, body));
});

test("private requests use only the Engine and selected JSON, with no credentials or provider calls in the body", async t => {
  const calls = [], body = reviewAiAgentSettings(settings());
  t.mock.method(globalThis, "fetch", (target, init) => { calls.push({ target, init }); return Response.json(init.body ? saved(body) : settings()); });
  assert.deepEqual(await requestAiAgentSettings(url, signal()), settings());
  assert.deepEqual(await requestAiAgentSettings(url, signal(), body), saved(body).settings);
  assert.equal(calls.length, 2);
  for (const { target, init } of calls) {
    assert.equal(target, url); assert.equal(init.credentials, "include"); assert.equal(init.cache, "no-store"); assert.equal(init.redirect, "error");
  }
  assert.equal(calls[0].init.method, "GET"); assert.equal(calls[0].init.body, undefined);
  assert.equal(calls[1].init.method, "POST"); assert.deepEqual(JSON.parse(calls[1].init.body), body);
});

test("conflict is typed, other HTTP/MIME errors are sanitized, and none retries", async t => {
  let calls = 0;
  for (const status of [400, 401, 403, 409, 413, 503]) {
    t.mock.method(globalThis, "fetch", () => { calls++; return Response.json({ message: "private provider detail" }, { status }); });
    await assert.rejects(requestAiAgentSettings(url, signal(), reviewAiAgentSettings(settings())), error => {
      assert.equal(error.name, status === 409 ? "AiAgentSettingsConflict" : "Error");
      assert.doesNotMatch(error.message, /private provider detail/); return true;
    });
  }
  t.mock.method(globalThis, "fetch", () => { calls++; return new Response("html", { headers: { "content-type": "text/html" } }); });
  await assert.rejects(requestAiAgentSettings(url, signal())); assert.equal(calls, 7);
});

test("read deadline ends waiting for ignored abort headers and observes late failures", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let fail, init;
  t.mock.method(globalThis, "fetch", (_url, options) => { init = options; return new Promise((_resolve, reject) => { fail = reject; }); });
  const pending = requestAiAgentSettings(url, signal());
  t.mock.timers.tick(AI_AGENT_READ_TIMEOUT_MS - 1); assert.equal(init.signal.aborted, false);
  t.mock.timers.tick(1); await assert.rejects(pending, { name: "TimeoutError" });
  fail(new Error("late failure")); await Promise.resolve(); assert.equal(init.signal.aborted, true);
});

test("write deadline bounds a stalled JSON body and never adopts a late valid acknowledgement", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] }); let release, init;
  const body = reviewAiAgentSettings(settings());
  t.mock.method(globalThis, "fetch", (_url, options) => { init = options; return { ok: true, status: 200,
    headers: new Headers({ "content-type": "application/json" }), json: () => new Promise(resolve => { release = resolve; }) }; });
  const pending = requestAiAgentSettings(url, signal(), body); await Promise.resolve();
  t.mock.timers.tick(AI_AGENT_SAVE_TIMEOUT_MS); await assert.rejects(pending, { name: "TimeoutError" });
  release(saved(body)); await Promise.resolve(); assert.equal(init.signal.aborted, true);
});

test("pre-abort prevents dispatch and later navigation abort ends a non-cooperative write", async t => {
  let release, calls = 0;
  t.mock.method(globalThis, "fetch", () => { calls++; return new Promise(resolve => { release = resolve; }); });
  const parent = new AbortController(); parent.abort();
  await assert.rejects(requestAiAgentSettings(url, parent.signal), { name: "AbortError" }); assert.equal(calls, 0);
  const next = new AbortController(), body = reviewAiAgentSettings(settings());
  const pending = requestAiAgentSettings(url, next.signal, body); next.abort();
  await assert.rejects(pending, { name: "AbortError" }); release(Response.json(saved(body))); await Promise.resolve(); assert.equal(calls, 1);
});

test("all four locale dictionaries have complete AI configuration and safety messages", () => {
  const flatten = (value, prefix = "") => Object.entries(value).flatMap(([key, item]) => typeof item === "string"
    ? [[`${prefix}${key}`, item]] : flatten(item, `${prefix}${key}.`));
  const source = Object.fromEntries(flatten(aiAgentMessages.en));
  for (const locale of ["zh-CN", "es", "ja"]) {
    const actual = Object.fromEntries(flatten(aiAgentMessages[locale]));
    assert.deepEqual(Object.keys(actual).sort(), Object.keys(source).sort());
    for (const [key, value] of Object.entries(actual)) {
      assert.ok(value.trim()); assert.deepEqual(value.match(/\{\w+\}/g), source[key].match(/\{\w+\}/g), `${locale}.${key}`);
    }
  }
});
