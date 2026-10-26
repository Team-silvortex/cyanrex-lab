// Retained storage HTTP guards are additive; the original pooled adapter guards stay separate.
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const ordered = (source, parts) => {
  let cursor = -1;
  for (const part of parts) {
    cursor = source.indexOf(part, cursor + 1);
    assert.ok(cursor >= 0, `present and ordered: ${part}`);
  }
};

test("retained publication HTTP derives origin only from its connector and shares bounded response admission", async () => {
  const source = code(await read("engine/src/platform_http/text_artifact_publish/mod.rs"));
  const dense = compact(source);
  assert.match(source, /Platform\(Arc<PlatformTextArtifactConnector>\)/);
  assert.match(source, /admission: Arc<Semaphore>/);
  assert.ok(dense.includes("Self::with_capacity(connector,4)"));
  ordered(dense, ["config::validate(connector.origin(),&[max_in_flight])?", "connector.origin().to_owned()",
    "Backend::Platform(Arc::new(connector))"]);
  assert.equal([...dense.matchAll(/\.route\(/g)].length, 1);
  assert.equal([...dense.matchAll(/\.fallback\(/g)].length, 1);
  assert.ok(dense.includes('"/platform/v1/text-artifacts"'));
  assert.ok(dense.includes("connector.create_session_text_artifact("));
  assert.doesNotMatch(source, /DurableAuthSource|SessionArtifactWorkspace|PgPool|std::env|install_empty|build_text_artifact_router|\.merge\(/);
});

test("retained text HTTP completes strict wire and profile validation before a single backend dispatch", async () => {
  const source = compact(await read("engine/src/platform_http/text_artifact_publish/handlers.rs"));
  ordered(source, ['request.uri().path()!="/platform/v1/text-artifacts"', "request.uri().query().is_some()", "Method::POST"]);
  ordered(source, ["endpoint(&request)", "security::credentials(", "admission::acquire(", "handle(state,token,request).await",
    "admission::response(response,permit)"]);
  ordered(source, ["security::bounded_request_bytes(request,false,text_contract::MAX_WIRE_BYTES)",
    "contract::parse(&bytes)?", "request.into_text_parts()?", ".backend.create(&token,artifact,revision,title,text)",
    "StatusCode::CREATED", "text_contract::TextResponse::from(committed)"]);
  assert.doesNotMatch(source, /timeout\(|security::request_bytes\(|SET_COOKIE|Set-Cookie|read_session_artifact|\.clone\(|serde_json::Value|retry|remove_file/);
});

test("pooled and retained text adapters share the original strict DTO and metadata rules without copying payload", async () => {
  const source = compact(await read("engine/src/platform_http/text_artifact/contract.rs"));
  assert.ok(source.includes("#[serde(deny_unknown_fields)]"));
  assert.ok(source.includes("MAX_WIRE_BYTES:usize=2*1024*1024"));
  ordered(source, ["fninto_text_parts(self)", "validate_text_bytes(self.text.as_bytes())",
    "validate_artifact_metadata(&self.title,TEXT_MEDIA,self.text.len()asu64)", "Ok((self.title,self.text))"]);
  assert.doesNotMatch(source, /self\.(?:title|text)\.clone\(|self\.text\.as_bytes\(\)\.to_vec\(/);
  const original = compact(await read("engine/src/platform_http/text_artifact/handlers.rs"));
  assert.ok(original.includes("request.into_draft()?"));
  assert.ok(original.includes(".create_session_artifact("));
  assert.ok(original.includes(".read_session_artifact("));
  assert.doesNotMatch(original, /PlatformTextArtifactConnector|TextArtifactPublishHttpState/);
});

test("retained text HTTP stays separate from public teaching and pooled routers with only explicit workspace opt-in", async () => {
  for (const file of ["engine/src/platform_host/session_pair.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(code(await read(file)), /TextArtifactPublishHttpState|build_text_artifact_publish_router|text_artifact_publish/, file);
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.ok(workspace.includes("text:Option<TextArtifactPublishHttpState>"));
  const old = workspace.slice(workspace.indexOf("pubfnwith_replace_capacities("), workspace.indexOf("pubfnwith_text_publication_capacities("));
  assert.ok(old.includes("text:None"));
  assert.doesNotMatch(old, /TextArtifactPublishHttpState|text_artifact_publish|split_platform_text_publication/);
  assert.doesNotMatch(workspace, /build_text_artifact_publish_router|build_text_artifact_router/);
});

test("retained publication HTTP SQL boundaries are enumerated once in the explicit CI runner", async () => {
  const directory = "engine/tests/platform_text_artifact_http", cases = [];
  const harness = await read("engine/tests/platform_text_artifact_http_tdd.rs");
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_text_artifact_http_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_text_artifact_http_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason] of ignored) assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`platform_text_artifact_http/${file}`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
  }
  assert.equal(cases.length, 6);
  const runner = await read("scripts/test-platform-retained-text-http.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_text_artifact_http_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, 6);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_text_artifact_http_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const step = (await read(".github/workflows/ci.yml"))
    .split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /&& bash scripts\/test-platform-retained-text-http\.sh/);
});

test("retained HTTP publication documentation is bilingual reachable and distinct from browser saving", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const file = `docs/${locale}/platform-text-artifact-connection.md`, guide = await read(file);
    for (const value of ["TextArtifactPublishHttpState", "build_text_artifact_publish_router", "POST", "/platform/v1/text-artifacts",
      "2 MiB", "256 KiB", "1–16", "not_attempted", "unconfirmed", "test-platform-retained-text-http.sh"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-text-artifact-connection.md`));
    await assertDocumentationReference(`docs/${locale}/README.md`, file);
  }
});
