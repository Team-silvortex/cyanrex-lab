// Composition inventories supplement actual SQL and native plain/TLS child tests.
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const compact = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "").replace(/\s+/g, "");
const body = (source, declaration) => {
  const start = source.indexOf(declaration);
  assert.ok(start >= 0, declaration);
  const opening = source.indexOf("{", start);
  let end = opening + 1, depth = 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    else if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0, declaration);
  return source.slice(start, end);
};

test("workspace publication validates seven lanes before one original split and preserves old defaults", async () => {
  const source = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.ok(source.includes("text:Option<TextArtifactPublishHttpState>"));
  const old = body(source, "pubfnwith_replace_capacities(");
  assert.ok(old.includes("text:None"));
  assert.doesNotMatch(old, /TextArtifactPublishHttpState|text_artifact_publish|split_platform_text_publication/);
  const added = body(source, "pubfnwith_text_publication_capacities(");
  const capacities = "login_capacity,lifecycle_capacity,index_capacity,content_capacity,create_capacity,replace_capacity,publish_capacity";
  assert.match(added, new RegExp(`config::validate\\(storage.origin\\(\\),&\\[${capacities},?\\],?\\)\\?;`));
  assert.ok(added.indexOf("config::validate(") < added.indexOf("split_platform_text_publication_connectors(storage)"));
  assert.ok(added.includes("text:Some(TextArtifactPublishHttpState::with_capacity(text,publish_capacity,)?),"));
  assert.doesNotMatch(source, /\.await|\.connect\(|PgPool|std::(?:env|fs)|build_text_artifact_router/);
  const router = body(source, "pubfnbuild_platform_workspace_router(");
  assert.equal([...router.matchAll(/\.merge\(/g)].length, 7);
  assert.equal([...router.split("ifletSome(text)=state.text")[0].matchAll(/\.merge\(/g)].length, 6);
  assert.ok(router.includes("router.merge(text_artifact_publish::routes().with_state(text))"));
  assert.equal([...router.matchAll(/\.fallback\(/g)].length, 1);
  assert.equal([...router.matchAll(/\.layer\(/g)].length, 1);
});

test("seven connectors share only one retained storage while text keeps its own actual worker budget", async () => {
  const source = compact(await read("engine/src/platform_host/session_pair.rs"));
  const split = body(source, "pub(crate)fnsplit_platform_text_publication_connectors(");
  assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, 6);
  assert.deepEqual([...split.matchAll(/(Platform\w+Connector)::from_shared\(/g)].map(match => match[1]),
    ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector",
      "PlatformTaskCreateConnector", "PlatformTaskReplaceConnector", "PlatformTextArtifactConnector"]);
  assert.ok(split.includes("PlatformTextArtifactConnector::from_shared(storage)"));
  assert.doesNotMatch(split, /inspect_|verify_|\.connect\(|std::(?:env|fs)|\.clone\(/);
  const text = compact(await read("engine/src/platform_host/text_artifact_connection.rs"));
  assert.ok(body(text, "pubfnnew(").includes("Self::from_shared(Arc::new(storage))"));
  const shared = body(text, "pub(super)fnfrom_shared(");
  assert.ok(shared.includes("storage,workers:Arc::new(Semaphore::new(4))"));
  assert.doesNotMatch(shared, /inspect_|verify_|\.connect\(|std::(?:env|fs)/);
});

test("plain and native TLS hosts opt in at the same startup point without widening transport or public APIs", async () => {
  const host = compact(await read("engine/src/bin/cyanrex-platform/serve_loopback/mod.rs"));
  const call = "PlatformWorkspaceHttpState::with_text_publication_capacities(storage,4,4,4,4,4,4,4,)";
  assert.equal(host.split(call).length - 1, 1);
  assert.ok(host.indexOf("open_platform_storage_expected(") < host.indexOf(call));
  assert.ok(host.indexOf(call) < host.indexOf("TcpListener::bind("));
  assert.ok(host.includes("run_with_transport(path,bind,process,None,notify_systemd,session_ui)"));
  assert.match(host, /run_with_transport\(path,bind,process,Some\(\(cert,key\)\),notify_systemd,session_ui,?\)/);
  const transport = compact(await read("engine/src/bin/cyanrex-platform/serve_loopback/http.rs"));
  for (const value of ["CONNECTIONS:usize=16", ".max_headers(32)", ".max_buf_size(16*1024)"])
    assert.ok(transport.includes(value), value);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(file), /with_text_publication_capacities|TextArtifactPublishHttpState|PlatformTextArtifactConnector/, file);
});

test("six real publication host boundaries are registered once and selected explicitly by CI", async () => {
  const directory = "engine/tests/platform_text_publication_host", cases = [];
  const harness = await read("engine/tests/platform_text_publication_host_tdd.rs");
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_text_publication_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_text_publication_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason] of ignored) assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`${directory.slice("engine/tests/".length)}/${file}`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
  }
  assert.equal(cases.length, 6);
  const runner = await read("scripts/test-platform-text-publication-host.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_text_publication_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, 6);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_text_publication_host_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /&& bash scripts\/test-platform-text-publication-host\.sh/);
});

test("bilingual host guides distinguish explicit publication from browser save and existing narrow constructors", async () => {
  for (const locale of ["en", "zh-CN"]) {
    for (const file of ["platform-text-artifact-connection.md", "platform-loopback-host.md", "platform-loopback-tls-host.md"]) {
      const guide = await read(`docs/${locale}/${file}`);
      for (const value of ["with_text_publication_capacities", "/platform/v1/text-artifacts"])
        assert.ok(guide.includes(value), `${locale}/${file}: ${value}`);
      assert.equal(guide, await read(`frontend/public/course/${locale}/${file}`));
    }
  }
});
