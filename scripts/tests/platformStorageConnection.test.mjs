import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("storage construction binds genuine original observations without discovering or opening a baseline", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_connection.rs"));
  for (const value of ["database: PlatformDatabaseConnector", "original_markers: PlatformStorageMarkerObservation",
    "original_root: PlatformArtifactRootObservation", "database.binds_catalog(&config, original_markers.catalog())",
    "original_root.binds_path(&config, root_path)", "root_path: root_path.to_owned()"])
    assert.ok(code.includes(value), value);
  const construction = code.split("pub fn new(")[1]?.split("pub async fn connect")[0] ?? "";
  assert.doesNotMatch(construction, /inspect_|verify_|\.connect\(|std::fs|std::env|dial\(/);
  const database = codeOnly(await read("engine/src/platform_host/database_connection.rs"));
  for (const value of ["pub(super) fn binds_catalog", "!observed.config_matches(config)",
    "OriginalCatalog::Observed(original) => original.matches(observed)",
    "OriginalCatalog::Expected(expected) => expected.matches(observed)"])
    assert.ok(database.includes(value), value);
  const artifact = codeOnly(await read("engine/src/platform_host/artifact_root.rs"));
  assert.ok(artifact.includes("path.as_os_str() == self.path.as_os_str()"));
  assert.doesNotMatch(code, /derive\([^\)]*(?:Debug|Clone|Serialize)[^\)]*\)\]\s*pub struct PlatformStorageConnector/);
});

test("fresh connection publication follows root checks and original marker verification with separate bounded cleanup", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_connection.rs"));
  const connect = (code.split("pub async fn connect")[1] ?? "").replace(/\s+/g, " ");
  const compact = connect.replace(/\s+/g, "");
  const before = connect.indexOf("verify_platform_artifact_root(");
  const database = connect.indexOf("let database = self.connect_database().await");
  const after = connect.indexOf("let root = verify_platform_artifact_root(");
  assert.ok(before >= 0 && before < database && database < after);
  assert.ok(connect.includes("match (database, root)"));
  assert.ok(connect.includes("(Ok(connection), Ok(())) => Ok(connection)"));
  assert.ok(connect.includes("close(connection).await"));
  assert.ok(compact.includes("self.database.connect().await"));
  for (const value of ["verify_platform_storage_markers(",
    "&self.original_markers", "connection.close()", "Duration::from_secs(10)"])
    assert.ok(connect.includes(value), value);
  assert.equal([...compact.matchAll(/\.database\.connect\(\)/g)].length, 1);
  assert.equal([...code.matchAll(/tokio::time::timeout\(Duration::from_secs\(10\)/g)].length, 2);
});

test("retained storage composition leaves process listeners pools issuers and existing diagnostics separate", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_connection.rs"));
  assert.doesNotMatch(code, /PgPool|PoolOptions|after_connect|TcpListener|AuthService|Engine|build_state|install_|inspect_platform|std::env|set_var|remove_var|query\(|Serialize|println!|tracing::/);
  const exports = await read("engine/src/platform_host/mod.rs");
  for (const name of ["PlatformStorageConnector", "PlatformStorageConnectionError"])
    assert.ok(exports.includes(name), name);
  for (const file of ["engine/src/bin/cyanrex-platform/main.rs", "engine/src/bin/cyanrex-platform/storage_check.rs",
    "engine/src/platform_host/storage_check.rs"])
    assert.ok(!(await read(file)).includes("PlatformStorageConnector"), file);
  const roots = await read("engine/tests/platform_storage_connection/roots.rs");
  for (const value of ["RootOpens::new(root.path())", "let precheck_opened = opens.observed()",
    "let post_failure_opened = opens.observed()", "let cancel_pending = tokio::select!", "drop(waiting)"])
    assert.ok(roots.includes(value), value);
});

test("four exact storage SQL selectors and bilingual scope documents remain wired into existing CI", async () => {
  const cases = [];
  for (const file of ["behavior", "roots"])
    for (const [, name] of (await read(`engine/tests/platform_storage_connection/${file}.rs`))
      .matchAll(/async fn (postgres_platform_storage_connection_\w+)\(/g)) cases.push(`${file}::${name}`);
  assert.equal(cases.length, 4);
  const runner = await read("scripts/test-platform-storage-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_storage_connection_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_storage_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-storage-connection\.sh/);
  const ciScripts = [...ci.matchAll(/bash (scripts\/[\w-]+\.sh)/g)].map(match => match[1]);
  assert.equal(new Set(ciScripts).size, ciScripts.length);
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-storage-connection.md`);
    for (const value of ["issues/3", "PlatformStorageConnector", "InvalidConfig", "Unavailable", "check-storage",
      "platform-catalog-expectation.md", "platform-storage-markers.md", "platform-process-startup.md", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), value);
    for (const file of ["README.md", "platform-database-connection.md", "platform-storage-check.md"])
      assert.ok((await read(`docs/${locale}/${file}`)).includes("platform-storage-connection.md"));
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-storage-connection.md`));
  }
});
