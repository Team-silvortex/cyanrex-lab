import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const directory = "engine/tests/catalog_task_content_http";
const target = "engine/tests/catalog_task_content_http_tdd.rs";
const sqlModules = ["behavior", "privacy", "metadata", "validation", "lifecycle", "faults", "waits", "capacity_sql"];

function fields(source, name) {
  const match = new RegExp(`struct\\s+${name}\\s*\\{([^}]+)\\}`).exec(source);
  assert.ok(match, `missing contract ${name}`);
  return match[1];
}
function names(source, name) {
  return [...fields(source, name).matchAll(/(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:/g)].map(match => match[1]);
}
async function sources(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

// Structural wiring and inventory guards only; Rust and real PostgreSQL prove behavior.
test("catalogue HTTP is trusted configuration with strict streaming exact-reference creation", async () => {
  const state = codeOnly(await read("engine/src/platform_http/catalog_task/mod.rs"));
  assert.deepEqual(names(state, "CatalogTaskContentHttpState"), ["source", "workspace", "origin", "admission"]);
  assert.match(fields(state, "CatalogTaskContentHttpState"), /workspace\s*:\s*SessionCatalogTaskContentWorkspace/);
  assert.doesNotMatch(fields(state, "CatalogTaskContentHttpState"), /\bpub\b/);
  assert.match(state, /pub\s+fn\s+build_catalog_task_content_router/);
  assert.match(state, /config::validate\(origin, &\[max_in_flight\]\)\?;/);
  assert.match(state, /admission: Arc::new\(Semaphore::new\(max_in_flight\)\)/);
  assert.ok(state.indexOf("config::validate(origin, &[max_in_flight])?") < state.indexOf("Semaphore::new(max_in_flight)"));
  const configuration = codeOnly(await read("engine/src/platform_http/config.rs"));
  assert.match(configuration, /security::validate_origin\(origin\)\?;[\s\S]*capacities\s*\.iter\(\)\s*\.any\([\s\S]*!\(1\.\.=16\)\.contains\(/);
  const contract = codeOnly(await read("engine/src/platform_http/catalog_task/contract.rs"));
  assert.deepEqual(names(contract, "Create"), ["schema_version", "task_id", "definition_ref", "manifest"]);
  assert.match(fields(contract, "Create"), /definition_ref\s*:\s*MapOnly\s*<\s*DefinitionRef\s*>/);
  assert.deepEqual(names(contract, "DefinitionRef"), ["package", "name", "version"]);
  assert.match(fields(contract, "DefinitionRef"), /package\s*:\s*MapOnly\s*<\s*VersionedName\s*>/);
  assert.equal([...contract.matchAll(/serde\(deny_unknown_fields\)/g)].length, 2);
  assert.doesNotMatch(contract, /serde_json\s*::\s*Value|\bfrom_value\b|\bTaskDefinition\b|TaskDefinitionCatalog/);
  const handler = codeOnly(await read("engine/src/platform_http/catalog_task/handlers.rs"));
  for (const name of ["create", "get", "replace", "transition"]) assert.equal([...handler.matchAll(new RegExp(`\\.${name}_session_catalog_content_task\\s*\\(`, "g"))].length, 1);
  for (const path of ["endpoint_at", "security::credentials", "security::request_bytes", "admission::acquire", "admission::response", "shared_contract::parse"]) assert.ok(handler.includes(path), path);
  assert.match(handler, /\/platform\/v1\/catalog-tasks/);
  assert.doesNotMatch(handler, /\.(?:create|get|replace|transition)_session_content_task\s*\(|\.assess\s*\(|\bTaskProvider\b|\bTaskAssessment\b|install_empty|domain_packs/);
  const errors = codeOnly(await read("engine/src/platform_http/errors.rs"));
  assert.match(errors, /TaskStoreError\s*::\s*UnknownDefinition/);
  assert.match(errors, /Self\s*::\s*dispatched\s*\(\s*status\s*,\s*code\s*\)/);
});

test("catalogue preparation does not promote manual publication or mount public API, SDK or browser access", async () => {
  const manual = await read("engine/src/platform_http/contract.rs");
  assert.deepEqual(names(manual, "Create"), ["schema_version", "task_id", "manifest"]);
  assert.match(await read("engine/src/platform_http/handlers.rs"), /\/platform\/v1\/tasks/);
  const files = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  files.push(...await sources("engine/src/routes", /\.rs$/), ...await sources("engine/src/services/auth_service/durable_source/draft_publication", /\.rs$/), ...await sources("sdk-js/src", /\.ts$/), ...await sources("frontend/src/features/tasks", /\.tsx?$/));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), /CatalogTaskContentHttpState|build_catalog_task_content_router|\/platform\/v1\/catalog-tasks|SessionCatalogTaskContentWorkspace|(?:create|get|replace|transition)_session_catalog_content_task/, path);
  for (const [path, source] of await sources("engine/src/platform_http/catalog_task", /\.rs$/)) assert.doesNotMatch(codeOnly(source), /\.\s*assess\s*\(|\b(?:TaskProvider|TaskAssessment|AssessmentOutcome|AuthService)\b|std\s*::\s*(?:env|net|process)|\.\s*(?:listen|bind|install_empty_namespace)\s*\(/, path);
});

test("catalogue HTTP defaults and eight opt-in SQL fixtures are fully mounted with exact CI inventory", async () => {
  const harness = await read(target);
  let defaults = 0;
  for (const module of ["defaults", "references", "bodies"]) {
    const path = `catalog_task_content_http/${module}.rs`;
    assert.ok(harness.includes(`#[path = "${path}"]\nmod ${module};`));
    const source = await read(`engine/tests/${path}`);
    assert.doesNotMatch(source, /#\[ignore\s*=/);
    defaults += [...source.matchAll(/#\[tokio::test\]/g)].length;
  }
  const shared = await read(`${directory}/shared.rs`);
  assert.ok(harness.includes('#[path = "catalog_task_content_http/shared.rs"]\nmod shared;'));
  for (const module of ["transport", "admission"]) {
    assert.ok(shared.includes(`#[path = "../platform_http/${module}.rs"]\nmod ${module};`));
    defaults += [...(await read(`engine/tests/platform_http/${module}.rs`)).matchAll(/#\[tokio::test\]/g)].length;
  }
  assert.equal(defaults, 28);
  const cases = [];
  for (const module of sqlModules) {
    const path = `catalog_task_content_http/${module}.rs`;
    assert.ok(harness.includes(`#[path = "${path}"]\nmod ${module};`));
    const source = await read(`engine/tests/${path}`);
    const tests = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"\]\s*async fn (postgres_catalog_http_\w+)\(/g)].map(match => `${module}::${match[1]}`);
    assert.equal(tests.length, 1, path);
    cases.push(...tests);
  }
  const allSql = (await sources(directory, /\.rs$/)).flatMap(([path, source]) => [...source.matchAll(/async fn (postgres_catalog_http_\w+)\(/g)].map(match => `${path.split("/").at(-1).replace(/\.rs$/, "")}::${match[1]}`));
  assert.deepEqual(allSql.sort(), cases.sort(), "no SQL fixture is omitted from default harness or runner");
  const runner = await read("scripts/test-platform-catalog-task-content-http.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_catalog_http_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.match(await read(".github/workflows/ci.yml"), /&& bash scripts\/test-platform-catalog-task-content-http\.sh/);
  assert.match(await read("scripts/tests/postgresCi.test.mjs"), /assert.equal\(cases.length, 8,/);
  for (const [path, source] of [[target, harness], ...await sources(directory, /\.rs$/)]) assert.ok(source.split("\n").length <= 601, `${path} exceeds the 600-line source ceiling`);
});
