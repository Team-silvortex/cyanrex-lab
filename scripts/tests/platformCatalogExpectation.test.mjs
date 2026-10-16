import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => source.replace(/\s+/g, "");

test("trusted catalog declarations remain distinct from genuine observations and bind all identities", async () => {
  const code = codeOnly(await read("engine/src/platform_host/catalog/expectation.rs"));
  for (const value of ["pub struct PlatformCatalogExpectation", "role_oid == 0", "database_owner_oid == 0",
    "namespace_oids.contains(&0)", "namespace_owner_oids.contains(&0)", "namespace_owner_oids.contains(&role_oid)",
    "role_oid == database_owner_oid", "namespace_oids[0] == namespace_oids[1]", "namespace_oids[0] == namespace_oids[2]",
    "namespace_oids[1] == namespace_oids[2]", "self.config == observed.config", "self.role_oid == observed.role_oid",
    "self.database_owner_oid == observed.database_owner_oid", "self.namespaces == observed.namespaces",
    "inspect_platform_database(config, connection).await?", "Ok(observed)"])
    assert.ok(code.includes(value), value);
  assert.ok(code.indexOf("!expected.config_matches(config)") < code.indexOf("let observed = inspect_platform_database"));
  assert.doesNotMatch(code, /derive\(|PlatformCatalogObservation\s*\{|std::env|std::fs|Serialize|Deserialize|query\(/);
  const exports = await read("engine/src/platform_host/mod.rs");
  for (const name of ["PlatformCatalogExpectation", "inspect_platform_database_expected", "check_platform_database_storage_expected"])
    assert.ok(exports.includes(name), name);
});

test("single-attempt connections and close-bounded diagnostics check immutable declarations without first-read adoption", async () => {
  const connector = codeOnly(await read("engine/src/platform_host/database_connection.rs"));
  for (const value of ["pub fn with_catalog_expectation", "!expectation.config_matches(&config)",
    "original: OriginalCatalog::Expected(expectation)", "OriginalCatalog::Expected(expected)",
    "inspect_platform_database_expected(&self.config, &mut connection, expected)"])
    assert.ok(connector.includes(value), value);
  assert.ok(connector.indexOf("inspect_platform_database_expected(&self.config") < connector.indexOf("Ok(connection)"));
  assert.doesNotMatch(connector, /PgPool|after_connect|connect_lazy|std::env|set_var|remove_var/);
  const storage = codeOnly(await read("engine/src/platform_host/storage_check.rs"));
  assert.ok(storage.indexOf("!expected.config_matches(config)") < storage.indexOf("let options = options"));
  assert.equal([...storage.matchAll(/dial\(&options\)/g)].length, 1);
  const inspect = storage.split("async fn inspect(")[1]?.split("\n}\n")[0] ?? "";
  const declared = inspect.split("Self::Catalog(expected) =>")[1]?.split("Self::Markers(expected) =>")[0] ?? "";
  assert.ok(compact(declared).includes("inspect_platform_database_expected(config,connection,expected).await?"));
  assert.doesNotMatch(declared, /inspect_platform_database\(|\.or_else\(/);
  assert.ok(storage.includes("inspected.and(closed)"));
  const cli = codeOnly(await read("engine/src/bin/cyanrex-platform/storage_check.rs"));
  for (const value of ["match &input.storage_expectation", "StorageExpectation::Catalog(expected)", "check_platform_database_storage_expected(",
    "check_platform_database_storage(", "drop(runtime)", "inspected.and(root).and(process)"])
    assert.ok(cli.includes(value), value);
  const selected = cli.split("StorageExpectation::Catalog(expected) =>")[1]?.split("StorageExpectation::Current =>")[0] ?? "";
  assert.ok(selected.includes("check_platform_database_storage_expected("));
  assert.doesNotMatch(selected, /check_platform_database_storage\(|\.or_else\(/);
});

test("strict private version two input forwards duplicate keys and never infers omitted catalog identities", async () => {
  const config = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  const expected = codeOnly(await read("engine/src/bin/cyanrex-platform/config/expected_catalog.rs"));
  const presence = codeOnly(await read("engine/src/bin/cyanrex-platform/config/presence.rs"));
  const selector = codeOnly(await read("engine/src/bin/cyanrex-platform/config/storage_identity.rs"));
  assert.match(config, /catalog_identity\s*:\s*Presence<CatalogIdentity>/);
  assert.ok(compact(config).includes("StorageExpectation::bind(input.format_version,input.catalog_identity,input.marker_identity,&config,)"));
  for (const value of ["(1,Presence::Absent,Presence::Absent)=>Ok(Self::Current)",
    "(2,Presence::Present(MapOnly(catalog)),Presence::Absent)=>", "catalog.bind(config).map(Self::Catalog)",
    "(3,Presence::Present(MapOnly(catalog)),Presence::Present(MapOnly(markers)))=>",
    "markers.bind(catalog.bind(config)?).map(Self::Markers)", "_=>Err(())"])
    assert.ok(compact(selector).includes(value), value);
  assert.ok(compact(presence).includes("MapOnly::<T>::deserialize(deserializer).map(Self::Present)"));
  for (const value of ["PlatformCatalogExpectation::new(",
    "self.role_oid", "self.database_owner_oid", "self.source.0.oid", "self.task.0.oid",
    "self.artifacts.0.oid", "self.source.0.owner_oid", "self.task.0.owner_oid", "self.artifacts.0.owner_oid"])
    assert.ok(expected.includes(value), value);
  assert.equal([...expected.matchAll(/deny_unknown_fields/g)].length, 2);
  assert.doesNotMatch(config + expected + presence + selector, /serde_json::Value|from_value|untagged|std::env|std::fs|ConnectOptions|inspect_platform|unwrap_or_default|Option<u32>/);
});

test("four exact SQL selections and bilingual declaration guides remain connected to CI and course copies", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_catalog_expectation", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_catalog_expectation/${file}`))
      .matchAll(/async fn (postgres_platform_catalog_expectation_\w+)\(/g)) cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 4);
  const runner = await read("scripts/test-platform-catalog-expectation.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_catalog_expectation_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_catalog_expectation_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-catalog-expectation\.sh/);
  const ciScripts = [...ci.matchAll(/bash (scripts\/[\w-]+\.sh)/g)].map(match => match[1]);
  assert.equal(new Set(ciScripts).size, ciScripts.length, "the shared CI chain must not duplicate a runner");
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-catalog-expectation.md`);
    for (const value of ["issues/3", "PlatformCatalogExpectation", "with_catalog_expectation", "catalog_identity",
      "role_oid", "database_owner_oid", "owner_oid", "check-config", "check-storage", "SCRAM", "OID",
      "platform-startup-input.md", "platform-storage-check.md", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), value);
    for (const file of ["README.md", "platform-startup-input.md", "platform-storage-check.md", "platform-database-connection.md"])
      assert.ok((await read(`docs/${locale}/${file}`)).includes("platform-catalog-expectation.md"));
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-catalog-expectation.md`));
  }
});
