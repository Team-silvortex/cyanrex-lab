import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

function fields(source, name) {
  const result = new RegExp(`pub\\s+struct\\s+${name}\\s*\\{([^}]+)\\}`).exec(source);
  assert.ok(result, `missing expected public contract: ${name}`);
  return result[1];
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

// These source guards protect wiring and inventory, not compiled decoding or authorization.
test("optional definition bindings are distinct from mandatory assessment output and manual identity", async () => {
  const task = await read("engine/src/models/collaboration/task.rs");
  const definition = fields(task, "TaskDefinition");
  for (const name of ["evidence_schema", "assessment_policy"]) {
    assert.match(definition, new RegExp(`pub\\s+${name}\\s*:\\s*Option\\s*<\\s*VersionedName\\s*>`));
  }
  assert.doesNotMatch(definition, /skip_serializing_if|default/, "absent automatic bindings must not disappear from serialization");
  const assessment = fields(task, "TaskAssessment");
  assert.match(assessment, /pub\s+policy\s*:\s*VersionedName\s*,/);
  assert.match(assessment, /pub\s+evidence_schema\s*:\s*VersionedName\s*,/);
  assert.doesNotMatch(assessment, /Option\s*</, "an assessment cannot use absent pins");
  const work = await read("engine/src/models/collaboration/work.rs");
  assert.match(fields(work, "TaskSnapshot"), /pub\s+definition\s*:\s*Option\s*<\s*TaskDefinition\s*>/);
  const definitions = codeOnly(await read("engine/src/services/task_catalog/definitions.rs"));
  assert.match(definitions, /definitions\s*:\s*Arc\s*<\s*\[TaskDefinition\]\s*>/);
  assert.doesNotMatch(definitions, /\b(?:TaskProvider|TaskAssessment|AssessmentOutcome|Deserialize|Serialize|assess)\b/);
});

test("both unbound-definition SQL cases are ignored by default and present in exact CI inventories", async () => {
  const entries = [
    {
      module: "assessment_binding", directory: "session_catalog_task", target: "session_catalog_task_tdd", runner: "test-session-catalog-tasks.sh", count: 20,
      name: "postgres_session_catalog_tasks_unbound_assessment_metadata_roundtrips_and_binding_drift_never_writes",
    },
    {
      module: "catalog_assessment_binding", directory: "session_task_content", target: "session_task_content_tdd", runner: "test-session-task-content.sh", count: 33,
      name: "postgres_session_content_catalog_unbound_assessment_roundtrips_and_binding_drift_never_writes",
    },
  ];
  const inventory = await read("scripts/tests/postgresCi.test.mjs");
  const workflow = await read(".github/workflows/ci.yml");
  for (const entry of entries) {
    const path = `${entry.directory}/${entry.module}.rs`;
    const source = await read(`engine/tests/${path}`);
    const target = await read(`engine/tests/${entry.target}.rs`);
    assert.ok(target.includes(`#[path = "${path}"]\nmod ${entry.module};`), path);
    const cases = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"\]\s*async fn (postgres_\w+)\(/g)].map(match => match[1]);
    assert.deepEqual(cases, [entry.name]);
    assert.match(source, /assert_ne!\(\s*added,\s*unbound/);
    assert.match(source, /assert_ne!\(\s*removed,\s*bound/);
    const runner = await read(`scripts/${entry.runner}`);
    const exact = `${entry.module}::${entry.name}`;
    assert.equal([...runner.matchAll(new RegExp(`^  ${exact}\\b`, "gm"))].length, 1);
    assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
    assert.match(runner, /--ignored --list/);
    assert.match(runner, /--ignored --exact --nocapture/);
    assert.ok(workflow.includes(`bash scripts/${entry.runner}`));
    assert.ok(inventory.includes(`["${path}", "${entry.module}::"]`));
    assert.ok(inventory.includes(`assert.equal(cases.length, ${entry.count},`));
  }
});

test("unbound metadata adds no assessment dispatch or public/manual publication promotion", async () => {
  for (const path of [
    "engine/src/services/auth_service/durable_source/task_commands/inputs/catalog.rs",
    "engine/src/services/auth_service/durable_source/task_commands/content_catalog.rs",
    "engine/src/services/task_store/draft.rs",
    "engine/src/services/task_store/content.rs",
    "engine/src/services/task_store/content_catalog.rs",
  ]) {
    const source = codeOnly(await read(path));
    assert.doesNotMatch(source, /\.\s*assess\s*\(|\bTaskAssessment\b|\bAssessmentOutcome\b/, path);
  }
  const files = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  files.push(...await sourceFiles("engine/src/platform_http", /\.rs$/), ...await sourceFiles("engine/src/services/auth_service/durable_source/draft_publication", /\.rs$/), ...await sourceFiles("sdk-js/src", /\.ts$/), ...await sourceFiles("frontend/src/features/tasks", /\.tsx?$/));
  const explicitCatalogHttp = new Set(["engine/src/platform_http/catalog_task/mod.rs", "engine/src/platform_http/catalog_task/contract.rs", "engine/src/platform_http/catalog_task/handlers.rs"]);
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /TaskDefinitionCatalog\s*::\s*compose|\.\s*assess\s*\(|\bTaskProvider\b|\bTaskAssessment\b|\bAssessmentOutcome\b/, path);
    if (!explicitCatalogHttp.has(path)) assert.doesNotMatch(codeOnly(source), /SessionCatalogTaskContentWorkspace|(?:create|get|replace|transition)_session_catalog_content_task/, path);
  }
});
