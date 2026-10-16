import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => source.replace(/\s+/g, "");

// Internal architecture checks supplement the unchanged format, SQL and transport regressions.
test("validated startup storage identity is one owned exhaustive policy rather than two optional bindings", async () => {
  const config = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  const identity = codeOnly(await read("engine/src/bin/cyanrex-platform/config/storage_identity.rs"));
  assert.match(config, /storage_expectation\s*:\s*StorageExpectation\b/);
  assert.doesNotMatch(config, /(?:catalog_expectation|marker_expectation)\s*:\s*Option\s*</);
  assert.match(identity, /enum\s+StorageExpectation\b/);
  for (const pattern of [/\bCurrent\b/, /\bCatalog\s*\(\s*PlatformCatalogExpectation\s*\)/,
    /\bMarkers\s*\(\s*PlatformStorageMarkerExpectation\s*\)/]) assert.match(identity, pattern);
  assert.doesNotMatch(identity, /derive\([^)]*(?:Debug|Clone|Serialize|Deserialize)|Option<Platform(?:Catalog|StorageMarker)Expectation>/);
  const cli = codeOnly(await read("engine/src/bin/cyanrex-platform/storage_check.rs"));
  assert.match(cli, /match\s+&?input\.storage_expectation/);
  assert.doesNotMatch(cli, /input\.(?:catalog_expectation|marker_expectation)|\.or_else\(/);
  assert.equal([...compact(cli).matchAll(/runtime\.block_on\(/g)].length, 1);
  for (const variant of ["Current", "Catalog", "Markers"])
    assert.match(cli, new RegExp(`StorageExpectation::${variant}\\b`));
  assert.ok(cli.includes("drop(runtime)") && cli.includes("inspected.and(root).and(process)"));
});

test("one private presence wrapper distinguishes absence from strict map records without requiring DTO defaults", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/config/presence.rs"));
  assert.match(code, /enum\s+Presence\s*<\s*T\s*>/);
  assert.match(code, /\bAbsent\b/);
  assert.match(code, /\bPresent\s*\(\s*MapOnly<T>\s*\)/);
  assert.match(code, /impl\s*<\s*T\s*>\s+Default\s+for\s+Presence<T>/);
  assert.doesNotMatch(code, /T\s*:\s*Default|derive\([^)]*Default/);
  assert.ok(compact(code).includes("MapOnly::<T>::deserialize(deserializer).map(Self::Present)"));
  assert.doesNotMatch(code, /deserialize_option|visit_none|visit_unit|serde_json::Value|from_value|unwrap_or_default/);
  const files = await Promise.all(["expected_catalog", "expected_markers"].map(name =>
    read(`engine/src/bin/cyanrex-platform/config/${name}.rs`)));
  for (const source of files) {
    assert.doesNotMatch(codeOnly(source), /enum\s+Expected(?:Marker)?Field|impl[^{}]*Deserializer|impl[^{}]*Deserialize<'de>\s+for/);
    assert.equal([...source.matchAll(/deny_unknown_fields/g)].length, 2);
    assert.match(source, /MapOnly<\w+>/);
  }
});

test("format-to-policy selection is centralized and pure while typed DTO binding remains format independent", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/config/storage_identity.rs"));
  const arms = [...code.matchAll(/\(\s*([123])\s*,([\s\S]*?)\)\s*=>/g)];
  assert.equal(arms.length, 3);
  for (const [version, present, absent] of [[1, 0, 2], [2, 1, 1], [3, 2, 0]]) {
    const arm = arms.find(match => Number(match[1]) === version)?.[2] ?? "";
    assert.equal([...arm.matchAll(/Presence::Present\b/g)].length, present, `v${version}`);
    assert.equal([...arm.matchAll(/Presence::Absent\b/g)].length, absent, `v${version}`);
  }
  assert.match(code, /_\s*=>\s*Err\(\(\)\)/);
  for (const variant of ["Current", "Catalog", "Markers"])
    assert.match(code, new RegExp(`Self::${variant}\\b|StorageExpectation::${variant}\\b`));
  const config = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  assert.match(config, /StorageExpectation::bind\s*\(/);
  assert.doesNotMatch(config, /input\.(?:catalog_identity|marker_identity)\.bind\(/);
  for (const name of ["expected_catalog", "expected_markers"]) {
    const dto = codeOnly(await read(`engine/src/bin/cyanrex-platform/config/${name}.rs`));
    assert.doesNotMatch(dto, /format_version|\bversion\s*:\s*u8|match\s*\(\s*version/);
    assert.match(dto, /fn\s+bind\s*\(/);
  }
  assert.doesNotMatch(code, /std::(?:env|fs)|\.await|\.connect\(|inspect_platform|query\(|serde_json::Value|from_value/);
});

test("one exhaustive diagnostic inspection stays between the sole dial and explicit close without panic branches", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_check.rs"));
  const inspect = code.split("async fn inspect(")[1]?.split("\n}\n")[0] ?? "";
  assert.ok(inspect.length > 0);
  assert.equal([...inspect.matchAll(/match\s+&?self\b/g)].length, 1);
  for (const variant of ["Current", "Catalog", "Markers"])
    assert.match(inspect, new RegExp(`Self::${variant}\\b`));
  assert.doesNotMatch(code, /unreachable!|panic!|\.unwrap\(|\.expect\(/);
  assert.doesNotMatch(inspect, /dial\(|\.connect\(|\.close\(|\.begin\(|\.commit\(|timeout\(/);
  const checked = compact(code.split("async fn check(")[1] ?? "");
  assert.equal([...checked.matchAll(/dial\(&options\)/g)].length, 1);
  assert.equal([...checked.matchAll(/connection\.close\(/g)].length, 1);
  const config = checked.indexOf("!expected.config_matches(config)");
  const options = checked.indexOf("letoptions=options(");
  const dial = checked.indexOf("dial(&options)");
  const inspected = checked.indexOf("expected.inspect(config,&mutconnection).await");
  const close = checked.indexOf("connection.close()");
  const outcome = checked.indexOf("inspected.and(closed)");
  assert.ok(config >= 0 && config < options && options < dial && dial < inspected
    && inspected < close && close < outcome);
  assert.ok(checked.includes("tokio::time::timeout(Duration::from_secs(10)"));
});

test("host and HTTP map one private neutral Origin policy without a reverse host-to-HTTP dependency", async () => {
  const lib = codeOnly(await read("engine/src/lib.rs"));
  assert.match(lib, /#\[cfg\(unix\)\]\s*(?:pub\(crate\)\s+)?mod\s+platform_origin\s*;/);
  assert.doesNotMatch(lib, /pub\s+mod\s+platform_origin/);
  const neutral = codeOnly(await read("engine/src/platform_origin.rs"));
  assert.match(neutral, /pub\(crate\)\s+fn\s+validate\s*\(/);
  assert.doesNotMatch(neutral, /\b(?:axum|http|services)::|platform_http|platform_host|std::(?:env|fs)|tokio|TcpListener|\.await/);
  const host = codeOnly(await read("engine/src/platform_host/config.rs"));
  assert.doesNotMatch(host, /platform_http/);
  assert.match(host, /platform_origin::validate\(\s*origin\s*\)/);
  assert.ok(host.includes("PlatformHostPreflightError::InvalidOrigin"));
  const http = codeOnly(await read("engine/src/platform_http/security.rs"));
  const wrapper = http.split("pub(crate) fn validate_origin(")[1]?.split("\n}\n")[0] ?? "";
  assert.match(wrapper, /platform_origin::validate\(\s*value\s*\)/);
  assert.ok(wrapper.includes("TaskContentHttpConfigError::InvalidOrigin"));
  assert.doesNotMatch(wrapper, /Url::parse|is_loopback\(|ascii_serialization/);
});
