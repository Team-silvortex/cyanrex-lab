import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Small source-wiring guards only; compiled tests and exact SQL supply behavior evidence.
function body(source, name) {
  const match = new RegExp(`\\bfn\\s+${name}\\b`).exec(source);
  assert.ok(match, `missing expected method: ${name}`);
  const start = source.indexOf("{", match.index + match[0].length);
  assert.ok(start >= 0);
  let depth = 1;
  for (let index = start + 1; index < source.length; index++) {
    if (source[index] === "{") depth++;
    if (source[index] === "}" && --depth === 0) return source.slice(start + 1, index);
  }
  assert.fail(`unclosed method: ${name}`);
}

async function sourceFiles(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sourceFiles(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

test("catalog composition bounds flattened immutable metadata and rejects exact duplicate pins", async () => {
  const source = codeOnly(await read("engine/src/services/task_catalog/definitions.rs"));
  assert.match(source, /definitions\s*:\s*Arc\s*<\s*\[TaskDefinition\]\s*>/);
  assert.match(source, /pub\s+fn\s+compose\s*\(\s*catalogs\s*:\s*&\[Self\]\s*\)/);
  const composed = body(source, "compose");
  assert.match(composed, /catalogs\.is_empty\(\)\s*\|\|\s*catalogs\.len\(\)\s*>\s*256/);
  assert.match(composed, /checked_add\s*\(\s*catalog\.definitions\.len\(\)\s*\)/);
  assert.match(composed, /filter\s*\(\s*\|count\|\s*\*count\s*<=\s*256\s*\)/);
  assert.ok(composed.indexOf("checked_add") < composed.indexOf("Vec::with_capacity"));
  assert.match(composed, /for\s+catalog\s+in\s+catalogs\s*\{\s*for\s+definition\s+in\s+catalog\.definitions\.iter\(\)/);
  assert.match(composed, /validate_definition\s*\(\s*&definition\.reference\.package\s*,\s*definition\s*\)\s*\?/);
  assert.match(composed, /any\s*\(\s*\|other\|\s*other\.reference\s*==\s*definition\.reference\s*\)/);
  assert.ok(composed.indexOf("duplicate definition") < composed.indexOf("definitions.push"));
  assert.match(composed, /definitions\.push\s*\(\s*definition\.clone\(\)\s*\)/);
  assert.match(composed, /definitions\s*:\s*definitions\.into\(\)/);
  assert.match(body(source, "get"), /&definition\.reference\s*==\s*reference/);
  assert.doesNotMatch(source, /\b(?:TaskProvider|Evidence|AssessmentOutcome|Deserialize|Serialize|assess|domain_packs)\b|std\s*::\s*(?:env|fs|process|net)\b|\.\s*(?:dedup|sort|retain)\s*\(/);
});

test("typed providers and manual/public composition retain their existing explicit boundaries", async () => {
  const source = codeOnly(await read("engine/src/services/task_catalog.rs"));
  const typed = body(source, "new");
  assert.match(typed, /TaskDefinitionCatalog\s*::\s*new\s*\(\s*package\s*,\s*provider\.definitions\(\)\s*\)/);
  assert.doesNotMatch(typed, /\bcompose\b/);
  const registration = body(codeOnly(await read("engine/src/services/task_catalog/definitions.rs")), "new");
  assert.match(registration, /definitions\.is_empty\(\)\s*\|\|\s*definitions\.len\(\)\s*>\s*256/);
  assert.match(registration, /validate_definition\s*\(\s*&package\s*,\s*definition\s*\)/);
  const manual = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  for (const name of ["create_session_content_task", "get_session_content_task", "replace_session_content_task", "transition_session_content_task", "create_content_task_in_transaction", "read_content_task_in_transaction"]) {
    const command = body(manual, name);
    assert.match(command, /\bNone\b/, name);
    assert.doesNotMatch(command, /TaskDefinitionCatalog|CatalogCreate|Some\s*\(/, name);
  }
  const files = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  files.push(...await sourceFiles("engine/src/platform_http", /\.rs$/), ...await sourceFiles("engine/src/services/auth_service/durable_source/draft_publication", /\.rs$/), ...await sourceFiles("sdk-js/src", /\.ts$/), ...await sourceFiles("frontend/src/features/tasks", /\.tsx?$/));
  const explicitCatalogHttp = new Set(["engine/src/platform_http/catalog_task/mod.rs", "engine/src/platform_http/catalog_task/contract.rs", "engine/src/platform_http/catalog_task/handlers.rs"]);
  for (const [path, content] of files) {
    assert.doesNotMatch(codeOnly(content), /TaskDefinitionCatalog\s*::\s*compose|\.\s*assess\s*\(|\bTaskProvider\b/, path);
    if (!explicitCatalogHttp.has(path)) assert.doesNotMatch(codeOnly(content), /(?:create|get|replace|transition)_session_catalog_content_task/, path);
  }
});

test("both composition SQL cases are mounted, ignored by default and selected exactly in CI inventories", async () => {
  const inventories = [
    {
      module: "composition", directory: "session_catalog_task", target: "session_catalog_task_tdd", runner: "test-session-catalog-tasks.sh", count: 20,
      name: "postgres_session_catalog_tasks_composed_packages_keep_exact_versions_and_isolated_admission",
    },
    {
      module: "catalog_composition", directory: "session_task_content", target: "session_task_content_tdd", runner: "test-session-task-content.sh", count: 33,
      name: "postgres_session_content_catalog_composed_packages_keep_text_versions_and_isolated_admission",
    },
  ];
  const ciGuard = await read("scripts/tests/postgresCi.test.mjs");
  const workflow = await read(".github/workflows/ci.yml");
  for (const entry of inventories) {
    const path = `engine/tests/${entry.directory}/${entry.module}.rs`;
    const source = await read(path);
    const target = await read(`engine/tests/${entry.target}.rs`);
    assert.ok(target.includes(`#[path = "${entry.directory}/${entry.module}.rs"]\nmod ${entry.module};`), path);
    const cases = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"\]\s*async fn (postgres_\w+)\(/g)].map(match => match[1]);
    assert.deepEqual(cases, [entry.name], "new SQL fixtures cannot silently run in default Cargo tests");
    assert.match(source, /TaskDefinitionCatalog::compose\(&catalogs\)/);
    assert.match(source, /drop\((?:metadata|\(metadata, catalogs\))\)/);
    const runner = await read(`scripts/${entry.runner}`);
    const exact = `${entry.module}::${entry.name}`;
    assert.equal([...runner.matchAll(new RegExp(`^  ${exact}\\b`, "gm"))].length, 1);
    assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
    assert.match(runner, /--ignored --list/);
    assert.match(runner, /--ignored --exact --nocapture/);
    assert.ok(workflow.includes(`bash scripts/${entry.runner}`));
    assert.ok(ciGuard.includes(`["${entry.directory}/${entry.module}.rs", "${entry.module}::"]`));
    assert.ok(ciGuard.includes(`assert.equal(cases.length, ${entry.count},`));
  }
});
