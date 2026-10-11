import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Deliberately small wiring guards, not a Rust parser or runtime authorization evidence.
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

test("schema-3 catalog content is explicit exact frozen metadata, not a relaxed manual handle", async () => {
  const store = codeOnly(await read("engine/src/services/task_store/content.rs"));
  const catalog = codeOnly(await read("engine/src/services/task_store/content_catalog.rs"));
  const commands = codeOnly(await read("engine/src/services/task_store/content_commands.rs"));
  assert.match(store, /definitions\s*:\s*Option\s*<\s*TaskDefinitionCatalog\s*>/);
  assert.match(body(store, "new"), /definitions\s*:\s*None/);
  const configured = body(catalog, "from_definitions");
  assert.match(configured, /Self\s*::\s*new\s*\(\s*pool\s*,\s*scope\s*\)/);
  assert.match(configured, /Some\s*\(\s*catalog\s*\.\s*clone\s*\(\s*\)\s*\)/);
  assert.match(body(catalog, "catalog_definition"), /\.\s*get\s*\(\s*reference\s*\)/);
  assert.match(body(catalog, "catalog_definition"), /TaskStoreError\s*::\s*UnknownDefinition/);
  assert.match(body(store, "validate_record"), /catalog\s*\.\s*get\s*\(\s*&definition\s*\.\s*reference\s*\)\s*==\s*Some\s*\(\s*definition\s*\)/);
  assert.match(body(store, "validate_record"), /_\s*=>\s*return\s+Err\s*\(\s*TaskStoreError\s*::\s*InvalidRecord\s*\)/);
  for (const name of ["create", "create_in_transaction"]) {
    assert.match(body(commands, name), /self\s*\.\s*definitions\s*\.\s*is_some\s*\(\s*\)/);
    assert.match(body(commands, name), /TaskStoreError\s*::\s*InvalidInput/);
  }
  assert.match(commands, /pub\s*\(\s*super\s*\)\s+async\s+fn\s+create_selected_in_transaction\b/);
  assert.doesNotMatch(commands, /pub\s*(?:\(\s*crate\s*\))?\s+async\s+fn\s+create_selected_in_transaction\b/);
  assert.doesNotMatch(catalog, /\b(?:TaskProvider|AssessmentOutcome|Deserialize|Serialize|assess|domain_packs)\b|std\s*::\s*(?:env|fs|process|net)\b/);
});

test("catalog Session content reuses the original source-owned guards, byte union and confirmed commit", async () => {
  const wrapper = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content_catalog.rs"));
  const content = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  assert.match(wrapper, /struct\s+SessionCatalogTaskContentWorkspace\s*\{\s*content\s*:\s*SessionTaskContentWorkspace\s*,\s*definitions\s*:\s*TaskDefinitionCatalog/);
  const configured = body(wrapper, "from_definitions");
  assert.match(configured, /SessionTaskContentWorkspace\s*::\s*new\s*\(\s*namespace\s*,\s*scope\s*,\s*artifacts\s*\)\s*\?/);
  assert.match(configured, /catalog\s*\.\s*clone\s*\(\s*\)/);
  assert.doesNotMatch(wrapper, /\b(?:Deserialize|Serialize|TaskProvider|validate_session|begin_private_work|query)\b|\.\s*(?:transaction|begin|commit)\s*\(/);
  for (const name of ["create_session_catalog_content_task", "get_session_catalog_content_task", "replace_session_catalog_content_task", "transition_session_catalog_content_task"]) {
    const command = body(wrapper, name);
    assert.match(command, /\.\s*session_content_task\s*\(/, name);
    assert.match(command, /&workspace\s*\.\s*content/, name);
    assert.match(command, /Some\s*\(\s*&workspace\s*\.\s*definitions\s*\)/, name);
  }
  const execute = body(content, "execute_content_task");
  assert.match(execute, /Some\s*\(\s*catalog\s*\)\s*=>\s*\{?\s*TaskContentStore\s*::\s*from_definitions\s*\(/);
  assert.match(execute, /None\s*=>\s*TaskContentStore\s*::\s*new\s*\(/);
  assert.match(execute, /\.\s*create_catalog_in_transaction\s*\(/);
  assert.match(execute, /content_checks\s*\(\s*old\s*\.\s*as_ref\s*\(\s*\)\s*,\s*&operation\s*\)/);
  assert.equal([...execute.matchAll(/workspace\s*\.\s*verify_contents\s*\(/g)].length, 2, "old/new bytes are checked before and after writes");
  assert.equal([...execute.matchAll(/store\s*\.\s*get_in_transaction\s*\(/g)].length, 2, "exact stored metadata is re-read after writes");
  assert.match(execute, /!=\s*Some\s*\(\s*&snapshot\s*\)/);
  const dispatch = body(content, "session_content_task");
  assert.ok(dispatch.indexOf("begin_private_work") < dispatch.indexOf("execute_content_task"));
  assert.ok(dispatch.indexOf("execute_content_task") < dispatch.indexOf("context.finish"));
  assert.ok(dispatch.indexOf("context.finish") < dispatch.indexOf("tx.commit"));
  assert.match(dispatch, /Duration\s*::\s*from_secs\s*\(\s*10\s*\)/);
});

test("manual content publication and HTTP remain distinct and catalog content stays unmounted", async () => {
  const content = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  for (const name of ["create_session_content_task", "get_session_content_task", "replace_session_content_task", "transition_session_content_task", "create_content_task_in_transaction", "read_content_task_in_transaction"]) {
    const manual = body(content, name);
    assert.match(manual, /(?:session_content_task|execute_content_task)\s*\(/, name);
    assert.match(manual, /\bNone\b/, name);
    assert.doesNotMatch(manual, /Some\s*\(|CatalogCreate|TaskDefinitionCatalog/, name);
  }
  const prepared = [...await sourceFiles("engine/src/platform_http", /\.rs$/), ...await sourceFiles("engine/src/services/auth_service/durable_source/draft_publication", /\.rs$/)];
  const explicitCatalogHttp = new Set(["engine/src/platform_http/catalog_task/mod.rs", "engine/src/platform_http/catalog_task/contract.rs", "engine/src/platform_http/catalog_task/handlers.rs"]);
  for (const [path, source] of prepared) {
    assert.doesNotMatch(codeOnly(source), /\.\s*assess\s*\(|\bTaskProvider\b|\bTaskAssessment\b/, path);
    if (!explicitCatalogHttp.has(path)) assert.doesNotMatch(codeOnly(source), /SessionCatalogTaskContentWorkspace|(?:create|get|replace|transition)_session_catalog_content_task/, path);
  }
  const publicFiles = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  publicFiles.push(...await sourceFiles("engine/src/routes", /\.rs$/), ...await sourceFiles("sdk-js/src", /\.ts$/), ...await sourceFiles("frontend/src/features/tasks", /\.tsx?$/));
  for (const [path, source] of publicFiles) assert.doesNotMatch(codeOnly(source), /SessionCatalogTaskContentWorkspace|(?:create|get|replace|transition)_session_catalog_content_task|TaskContentStore\s*::\s*from_definitions/, path);
});
