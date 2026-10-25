import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = value => value.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// These source/inventory checks supplement real local probes and restricted-role SQL tests.
test("expected storage opening is explicit and leaves diagnostics and live composition unchanged", async () => {
  const source = code(await read("engine/src/platform_host/storage_open.rs"));
  assert.match(source, /pub async fn open_platform_storage_expected\(/);
  for (const name of ["PlatformHostPreflightConfig", "PlatformDatabaseEndpoint", "PlatformStorageMarkerExpectation",
    "PlatformArtifactRootObservation", "PlatformStorageConnector", "PlatformStorageConnectionError"])
    assert.ok(source.includes(name), name);
  assert.match(await read("engine/src/platform_host/mod.rs"), /pub use storage_open::open_platform_storage_expected/);
  assert.doesNotMatch(source, /PgPool|PoolOptions|TcpListener|Engine|AuthService|build_router|platform_http|std::env|set_var|remove_var|install_|println!|eprintln!|tracing::/);
  for (const file of ["engine/src/bin/cyanrex-platform/main.rs", "engine/src/bin/cyanrex-platform/storage_check.rs",
    "engine/src/platform_host/storage_check.rs", "engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.ok(!(await read(file)).includes("open_platform_storage_expected"), file);
});

test("prepared options and consumed declarations stay private without a second environment read", async () => {
  const database = code(await read("engine/src/platform_host/database_connection.rs"));
  const prepared = database.split("pub(super) fn from_expected_options(")[1]?.split("pub(super) fn binds_catalog(")[0] ?? "";
  assert.ok(prepared.includes("options: PgConnectOptions"));
  assert.ok(prepared.includes("expectation.config_matches(&config)"));
  assert.ok(prepared.includes("OriginalCatalog::Expected(expectation)"));
  assert.doesNotMatch(prepared, /new_without_pgpass|\boptions\(|\.connect\(|\.await|std::env/);
  assert.doesNotMatch(database, /pub fn from_expected_options|pub\(crate\) fn from_expected_options/);
  const expected = code(await read("engine/src/platform_host/storage_markers/expectation.rs"));
  assert.match(expected, /pub\(in crate::platform_host\) fn into_catalog\(self\) -> PlatformCatalogExpectation/);
  assert.doesNotMatch(expected, /pub fn into_catalog|derive\([^)]*(?:Clone|Debug|Serialize)|pub fn (?:catalog|markers|into_observation)/);
  const opener = code(await read("engine/src/platform_host/storage_open.rs"));
  assert.equal([...opener.matchAll(/\boptions\(/g)].length, 1);
  assert.equal([...opener.matchAll(/\bdial\(/g)].length, 1);
  assert.match(opener, /inspect_platform_storage_markers_expected\(/);
  assert.doesNotMatch(opener, /inspect_platform_database\(|inspect_platform_storage_markers\(|inspect_platform_artifact_root\(|\.or_else\(/);
  assert.match(opener, /connection\.close\(\)/);
  assert.match(opener, /Duration::from_secs\(10\)/);
  let previous = -1;
  for (const value of ["expected.config_matches(&config)", "original_root.binds_path(&config, root_path)",
    "verify_platform_artifact_root(", "let options = options(", "tokio::time::timeout(", "dial(&options)",
    "inspect_platform_storage_markers_expected(", "connection.close()", "let root = verify_platform_artifact_root(",
    "let markers = inspected?", "root?", "PlatformDatabaseConnector::from_expected_options(",
    "PlatformStorageConnector::new("]) {
    const current = opener.indexOf(value, previous + 1);
    assert.ok(current > previous && current >= 0, `ordered: ${value}`);
    previous = current;
  }
});

test("CI selects all seven expected-storage SQL cases exactly without fixture installation", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_storage_open", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_storage_open/${file}`))
      .matchAll(/async fn (postgres_platform_storage_open_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 7, "new SQL boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-storage-open.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_storage_open_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_storage_open_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-storage-open\.sh/);
});

test("bilingual expected-storage guides separate retained capability from diagnostics and a host", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-storage-open.md`);
    for (const value of ["issues/3", "open_platform_storage_expected", "PlatformStorageMarkerExpectation",
      "PlatformArtifactRootObservation", "InvalidConfig", "Unavailable", "PGOPTIONS", "Result<()>",
      "check-storage", "CYANREX_TEST_DATABASE_URL", "platform-storage-connection.md", "platform-process-startup.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /starts no host/ : /不启动宿主/);
    assert.match(guide, locale === "en" ? /Cancelling the future/ : /取消 future/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-storage-open.md`));
    for (const file of ["README.md", "platform-storage-connection.md", "architecture-map.md", "testing-guide.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-storage-open.md`);
  }
});
