import assert from "node:assert/strict";
import test from "node:test";
import { CyanrexClient } from "../dist/index.js";
import { validateAiAgentProfile, validateAiAgentSettings, resolveAgentCredential } from "../dist/agents/index.js";

const profile = (overrides = {}) => ({ id: "local-model", name: "Local model", protocol: "openai_responses",
  base_url: "http://127.0.0.1:11434/v1", model: "example-model", credential_ref: null, enabled: true, ...overrides });
const settings = (overrides = {}) => ({ revision: 0, default_profile_id: "local-model", profiles: [profile()], ...overrides });

test("profile metadata supports each provider protocol and local loopback models", () => {
  for (const protocol of ["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "custom"]) {
    assert.equal(validateAiAgentProfile(profile({ protocol })).protocol, protocol);
  }
  for (const base_url of ["https://provider.example/api/v1", "https://provider.example/path@name", "http://localhost:8123/path@name",
    "http://LOCALHOST:8123/v1", "http://[::1]:11434/api", "http://127.0.0.1:1234/v1"]) {
    assert.equal(validateAiAgentProfile(profile({ base_url })).base_url, base_url);
  }
  assert.equal(validateAiAgentProfile(profile({ name: "中".repeat(128), model: "😀".repeat(128) })).name.length, 128);
  assert.equal(validateAiAgentSettings(settings()).revision, 0);
});

test("profile configuration rejects secrets, unsafe endpoints and unexpected fields", () => {
  for (const invalid of [
    { api_key: "not-storable" }, { token: "not-storable" }, { headers: {} }, { credential_ref: "sk-secret" },
    { credential_ref: "env:KEY" }, { credential_ref: "key" }, { credential_ref: "KEY".repeat(30) },
    { name: " " }, { name: "a\u0085b" }, { name: "\ud800" }, { name: "x".repeat(129) }, { model: "" }, { enabled: "true" },
    { id: "../profile" }, { id: "" }, { protocol: "unknown" }, { protocol: ["custom"] },
    ...["http://192.168.1.2/v1", "https://user:pass@host.example", "https://host.example?key=secret",
      "https://host.example#token", "file:///tmp/model", "https://*.example/v1", "https://provider.example\n",
      "http://127.1/v1", "http://2130706433/v1", "http://127.1.2.3/v1", "https:foo", "https://host.example?",
      "https://host.example#", "https://host.example/a b", "https://host.example\\path",
      "https://host.example:/v1", "https://[::1]:/v1", "http://localhost:/v1"].map(base_url => ({ base_url })),
  ]) assert.throws(() => validateAiAgentProfile(profile(invalid)), TypeError, JSON.stringify(Object.keys(invalid)));
});

test("configuration bounds revisions, profile count, duplicate IDs and default availability", () => {
  for (const invalid of [{ revision: -1 }, { revision: 0x1_0000_0000 }, { revision: 0.5 }, { revision: "0" },
    { profiles: Array.from({ length: 17 }, (_, index) => profile({ id: `p${index}` })) },
    { profiles: [profile(), profile()] }, { default_profile_id: "absent" }, { profiles: [profile({ enabled: false })] },
    { extra: true }]) assert.throws(() => validateAiAgentSettings(settings(invalid)), TypeError);
  assert.equal(validateAiAgentSettings(settings({ revision: 0xffff_ffff })).revision, 0xffff_ffff);
  assert.deepEqual(validateAiAgentSettings({ revision: 0, default_profile_id: null, profiles: [] }).profiles, []);
  assert.throws(() => validateAiAgentSettings(settings({ default_profile_id: null,
    profiles: Array.from({ length: 16 }, (_, index) => profile({ id: `p${index}`,
      base_url: "https://provider.example/" + "x".repeat(1900), name: "中".repeat(128), model: "😀".repeat(128) })) })), TypeError);
});

test("credential lookup is explicit host-only and never part of stored configuration", async () => {
  const source = profile({ credential_ref: "MODEL_API_KEY" }); const references = [];
  const credential = await resolveAgentCredential(source, reference => { references.push(reference); return "synthetic-host-secret"; });
  assert.equal(credential, "synthetic-host-secret");
  assert.deepEqual(references, ["MODEL_API_KEY"]);
  assert.equal(JSON.stringify(source).includes("synthetic-host-secret"), false);
  assert.equal(await resolveAgentCredential(profile(), () => { throw Error("must not run"); }), null);
  await assert.rejects(resolveAgentCredential(profile({ enabled: false }), () => "secret"), TypeError);
  await assert.rejects(resolveAgentCredential(source, () => ""), TypeError);
});

test("AI settings SDK namespace preserves revision fencing, current Session and CSRF", async () => {
  const seen = []; const next = settings({ revision: 1 });
  const client = new CyanrexClient("https://engine.example", { sessionCookie: "synthetic", csrfOrigin: "https://teacher.example", fetch: async (url, init) => {
    seen.push({ url, init }); return Response.json(init.method === "GET" ? settings() : { ok: true, settings: next });
  } });
  const signal = new AbortController().signal;
  assert.deepEqual(await client.aiAgents.settings({ signal }), settings());
  const update = { expected_revision: 0, default_profile_id: "local-model", profiles: [profile()] };
  assert.deepEqual(await client.aiAgents.updateSettings(update, { signal }), { ok: true, settings: next });
  assert.equal(seen[0].url, "https://engine.example/settings/ai-agents");
  assert.equal(seen[1].init.method, "POST"); assert.equal(seen[1].init.signal, signal);
  assert.equal(seen[1].init.headers.Origin, "https://teacher.example");
  assert.equal(seen[1].init.headers.Cookie, "cyanrex_session=synthetic");
  for (const { init } of seen) {
    assert.equal(init.redirect, "error"); assert.equal(init.cache, "no-store"); assert.equal(init.referrerPolicy, "no-referrer");
  }
  assert.deepEqual(JSON.parse(seen[1].init.body), update);
});

test("stale AI settings writes remain errors and are not silently retried", async () => {
  let requests = 0;
  const client = new CyanrexClient("https://engine.example", { fetch: async () => { requests++; return Response.json({ message: "stale revision" }, { status: 409 }); } });
  await assert.rejects(client.aiAgents.updateSettings({ expected_revision: 0, default_profile_id: null, profiles: [] }), error => error.status === 409);
  assert.equal(requests, 1);
});
