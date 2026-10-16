import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => source.replace(/\s+/g, "");

// These inventories cannot prove SQL isolation, peer trust or a deployed host.
test("opaque marker declarations own a complete catalog assertion without reading or exporting a baseline", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_markers/expectation.rs"));
  for (const value of ["pub struct PlatformStorageMarkerExpectation", "catalog: PlatformCatalogExpectation",
    "markers: [MarkerIdentity; 5]", "marker_oids.contains(&0)", "marker_owner_oids.contains(&0)",
    "marker_owner_oids.contains(&catalog.role_oid())", "marker_oids[..index].contains(oid)",
    "self.catalog.config_matches(config)"])
    assert.ok(code.includes(value), value);
  assert.deepEqual([...code.matchAll(/pub fn (\w+)\(/g)].map(match => match[1]), ["new"]);
  assert.doesNotMatch(code, /^\s*pub\s+(?:catalog|markers)\s*:/m);
  assert.doesNotMatch(code, /derive\(|PlatformStorageMarkerObservation\s*\{|PlatformCatalogObservation\s*\{|std::env|std::fs|PgPool|Serialize|Deserialize/);
  const constructor = code.split("pub fn new(")[1]?.split("pub(in crate::platform_host)")[0] ?? "";
  assert.doesNotMatch(constructor, /inspect_|collect\(|query\(|\.connect\(|\.await/);
  const exports = await read("engine/src/platform_host/mod.rs");
  for (const name of ["PlatformStorageMarkerExpectation", "inspect_platform_storage_markers_expected",
    "check_platform_database_storage_markers_expected"])
    assert.ok(exports.includes(name), name);
});

test("all marker shape and declared identity checks precede every marker body read", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_markers.rs"));
  const body = code.includes("async fn collect_bound(")
    ? code.split("async fn collect_bound(")[1]?.split("async fn shape(")[0] ?? ""
    : code.split("async fn collect(")[1]?.split("async fn shape(")[0] ?? "";
  assert.ok(body.includes("CatalogBinding"));
  assert.match(code, /Observed\(&[^\n]*PlatformCatalogObservation\)|Observed\([^\n]*PlatformCatalogObservation/);
  assert.match(code, /Declared\(&[^\n]*PlatformCatalogExpectation\)|Declared\([^\n]*PlatformCatalogExpectation/);
  assert.match(body, /Option<&\[MarkerIdentity; 5\]>/);
  const sequence = compact(body);
  const lock = sequence.indexOf('query(&format!("LOCKTABLE{candidates}INACCESSSHAREMODE"))');
  const catalog = sequence.indexOf("letobserved_catalog=catalog::collect(config,tx).await?");
  const shapes = sequence.indexOf("formarkerin&MARKERS{letidentity=shape(config,tx,marker,namespace_oids[marker.namespace]).await?;markers.push(identity);}");
  const array = sequence.indexOf("letmarkers:[MarkerIdentity;5]=markers.try_into()");
  const identities = sequence.indexOf("ifexpected_markers.is_some_and(|expected|expected!=&markers){returnErr(PlatformHostPreflightError::StorageMarkersChanged);}");
  const bodies = sequence.indexOf("for(marker,identity)inMARKERS.iter().zip(markers){read_marker(");
  assert.ok(lock >= 0 && lock < catalog && catalog < shapes && shapes < array
    && array < identities && identities < bodies);
  assert.equal([...sequence.matchAll(/read_marker\(/g)].length, 1);
  for (const name of ["pub async fn verify_platform_storage_markers(", "pub(super) async fn verify_in_transaction("]) {
    const retained = compact(code.split(name)[1]?.split("async fn ")[0] ?? "");
    assert.ok(retained.includes("CatalogBinding::Observed(&expected.catalog)"));
    assert.ok(retained.includes("Some(&expected.markers)"));
  }
  const expected = compact(codeOnly(await read("engine/src/platform_host/storage_markers/expectation.rs")));
  assert.ok(expected.includes("CatalogBinding::Declared(&expected.catalog),Some(&expected.markers)"));
  const diagnostic = codeOnly(await read("engine/src/platform_host/storage_check.rs"));
  assert.ok(diagnostic.includes("check_platform_database_storage_markers_expected("));
  assert.ok(diagnostic.includes("inspect_platform_storage_markers_expected("));
  assert.equal([...diagnostic.matchAll(/dial\(&options\)/g)].length, 1);
  assert.ok(diagnostic.includes("inspected.and(closed)"));
  assert.doesNotMatch(diagnostic, /PgPool|after_connect|connect_lazy|\.or_else\(|std::env|install_/);
});

test("private format three strictly consumes catalog and all five named marker identities without fallback", async () => {
  const config = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  const field = codeOnly(await read("engine/src/bin/cyanrex-platform/config/expected_markers.rs"));
  const presence = codeOnly(await read("engine/src/bin/cyanrex-platform/config/presence.rs"));
  const selector = codeOnly(await read("engine/src/bin/cyanrex-platform/config/storage_identity.rs"));
  for (const value of ["marker_identity: Presence<MarkerIdentity>", "storage_expectation: StorageExpectation",
    "16 * 1024", "mod marker_expectation_tests;"])
    assert.ok(config.includes(value), value);
  assert.ok(compact(presence).includes("MapOnly::<T>::deserialize(deserializer).map(Self::Present)"));
  for (const value of ["(1,Presence::Absent,Presence::Absent)=>Ok(Self::Current)",
    "(2,Presence::Present(MapOnly(catalog)),Presence::Absent)=>", "catalog.bind(config).map(Self::Catalog)",
    "(3,Presence::Present(MapOnly(catalog)),Presence::Present(MapOnly(markers)))=>",
    "markers.bind(catalog.bind(config)?).map(Self::Markers)", "_=>Err(())"])
    assert.ok(compact(selector).includes(value), value);
  assert.ok(field.includes("PlatformStorageMarkerExpectation::new("));
  for (const marker of ["source", "identity", "access", "task", "artifacts"])
    for (const suffix of ["oid", "owner_oid"])
      assert.ok(field.includes(`self.${marker}.0.${suffix}`));
  for (const suffix of ["oid", "owner_oid"])
    assert.ok(compact(field).includes(`[${["source", "identity", "access", "task", "artifacts"]
      .map(marker => `self.${marker}.0.${suffix},`).join("")}]`));
  assert.equal([...field.matchAll(/deny_unknown_fields/g)].length, 2);
  assert.doesNotMatch(config + field + presence + selector, /serde_json::Value|from_value|untagged|std::env|std::fs|inspect_platform|unwrap_or_default/);
  const cli = codeOnly(await read("engine/src/bin/cyanrex-platform/storage_check.rs"));
  const selected = cli.split("StorageExpectation::Markers(expected) =>")[1]?.split("StorageExpectation::Catalog(expected) =>")[0] ?? "";
  assert.ok(selected.includes("check_platform_database_storage_markers_expected("));
  assert.doesNotMatch(selected, /check_platform_database_storage(?:_expected)?\(/);
  assert.ok(cli.includes("drop(runtime)") && cli.includes("inspected.and(root).and(process)"));
});

test("five exact marker declaration SQL selections and pure default targets are connected to disposable CI", async () => {
  assert.ok((await read("engine/tests/platform_marker_expectation_values.rs")).includes("PlatformStorageMarkerExpectation::new("));
  assert.ok((await read("engine/src/bin/cyanrex-platform/config/marker_expectation_tests.rs")).includes("platform_config_marker_expectation_accepts_complete_version_three"));
  const target = await read("engine/tests/platform_marker_expectation_tdd.rs");
  assert.ok(target.includes("platform_marker_expectation/"));
  assert.doesNotMatch(target, /platform_storage_markers_tdd\.rs|platform_catalog_expectation_tdd\.rs/);
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_marker_expectation", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = codeOnly(await read(`engine/tests/platform_marker_expectation/${file}`));
    for (const [, name] of source.matchAll(/^\s*async fn (postgres_platform_marker_expectation_\w+)\(/gm))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 5);
  const runner = await read("scripts/test-platform-marker-expectation.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_marker_expectation_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_marker_expectation_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-marker-expectation\.sh/);
  const runners = [...ci.matchAll(/bash (scripts\/[\w-]+\.sh)/g)].map(match => match[1]);
  assert.equal(new Set(runners).size, runners.length);
});

test("bilingual marker declaration guides and generated navigation retain diagnostic compatibility boundaries", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-marker-expectation.md`);
    for (const value of ["issues/3", "PlatformStorageMarkerExpectation", "marker_identity", "catalog_identity",
      "oid", "owner_oid", "check-config", "check-storage", "platform-catalog-expectation.md",
      "platform-storage-check.md", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), value);
    assert.ok((await read(`docs/${locale}/README.md`)).includes("platform-marker-expectation.md"));
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-marker-expectation.md`));
  }
});
