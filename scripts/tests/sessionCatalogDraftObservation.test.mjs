import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const base = "engine/src/services/auth_service/durable_source/";
const production = `${base}catalog_draft_publication/`;
const integration = "engine/tests/session_catalog_draft_observation/";
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory, extension) {
  const files = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) files.push(...await sources(path, extension));
    else if (extension.test(entry.name)) files.push([path, await read(path)]);
  }
  return files;
}

test("catalogue observation borrows only the original unknown attempt and delegates one existing reader", async () => {
  const code = codeOnly(await read(`${production}observation.rs`));
  assert.match(code, /impl SessionCatalogTaskDraftPublication/);
  assert.match(code, /pub async fn observe_unconfirmed\(\s*&self,\s*token: &str/);
  assert.equal([...code.matchAll(/pub (?:async )?fn /g)].length, 1);
  assert.ok(code.indexOf("SessionDraftPublicationState::Unconfirmed") < code.indexOf("!valid_token(token)"));
  assert.match(code, /fingerprint\(token\) != self\.fingerprint/);
  for (const command of ["read_session_artifact", "get_session_catalog_content_task"]) {
    assert.equal([...code.matchAll(new RegExp(`\\b${command}\\s*\\(`, "g"))].length, 1, command);
  }
  assert.match(code, /read_session_artifact\(token, self\.workspace\.publication_artifacts\(\), reference\)/);
  assert.match(code, /get_session_catalog_content_task\(token, &self\.workspace, reference\.task_id\)/);
  assert.doesNotMatch(code, /\b(?:get_session_content_task|create_session_artifact|create_session_catalog_content_task|replace_session_catalog_content_task|TaskContentStore|sqlx|query|query_as|query_scalar|spawn|install_empty_namespace|TaskProvider)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/);
});

test("catalogue comparisons require full frozen metadata ordered current bytes and never swallow reader errors", async () => {
  const code = codeOnly(await read(`${production}observation.rs`));
  assert.equal([...code.matchAll(/\.await\?/g)].length, 2, "both existing reader errors stay typed");
  assert.match(code, /artifact_matches_planned_create\(index, &content\.revision\)/);
  assert.match(code, /content\.bytes == item\.text\(\)\.as_bytes\(\)/);
  assert.match(code, /\.items\(\)\s*\.get\(index\)/);
  assert.match(code, /task_matches_planned_create\(expected, &content\.snapshot\)/);
  assert.match(code, /content\.contents\.len\(\) == self\.plan\.items\(\)\.len\(\)/);
  assert.match(code, /content\.contents\.iter\(\)\.enumerate\(\)\.all/);
  assert.match(code, /self\.artifact_observation\(index, Some\(item\)\)/);
  assert.match(await read(`${production}confirmation.rs`), /snapshot\.task\.definition\.as_ref\(\) == Some\(&self\.definition\)/);
  assert.match(await read("engine/src/services/task_store/content.rs"), /catalog\.get\(&definition\.reference\) == Some\(definition\)/);
  assert.doesNotMatch(code, /\.unwrap_or|\.or_else|\.ok\(\)|map_err\(|Err\([^)]*\)\s*=>/);
});

test("body-free catalogue observation cannot append receipts mutate progress or gain recovery authority", async () => {
  const code = codeOnly(await read(`${production}observation.rs`));
  assert.match(code, /Ok\(SessionDraftPublicationObservation\s*\{\s*step: step\.clone\(\),\s*result,/);
  assert.doesNotMatch(code, /self\.(?:state|confirmed|completed|definition|workspace|source|fingerprint|allocated)\s*=|self\.[\w.]+\.(?:push|clear|insert|remove|take|replace)\(/);
  assert.doesNotMatch(code, /\b(?:SessionTaskDraftPublication|SessionDraftPublicationCheckpoint|SessionDraftPublicationJournal|Deref|Serialize|Deserialize)\b/);
  const reports = codeOnly(await read(`${base}draft_publication/observation.rs`));
  assert.match(reports, /pub struct SessionDraftPublicationObservation\s*\{\s*pub step: SessionDraftPublicationStep,\s*pub result: SessionDraftObservedOutcome,/);
  assert.doesNotMatch(reports, /SessionCatalogTaskDraftPublication/);
  const attempt = codeOnly(await read(`${production}mod.rs`));
  assert.doesNotMatch(attempt, /pub\s+(?:async\s+)?fn\s+(?:restore|resume|reset|retry|checkpoint|into_journaled|accept_receipt|cleanup)\b/);
});

test("catalogue observation remains independent of HTTP application SDK and all manual recovery surfaces", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"].map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/], ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  files.push(...(await sources(`${base}draft_publication`, /\.rs$/)).filter(([path]) => !/(?:tests|test_support)/.test(path)));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), /\b(?:SessionCatalogTaskDraftPublication|prepare_catalog_task_draft_publication|catalog_draft_publication)\b/, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), /SessionCatalogTaskDraftPublication|prepare_catalog_task_draft_publication/);
});

test("catalogue observation default pure and SQL evidence are mounted with full preservation snapshots", async () => {
  const target = await read("engine/tests/session_catalog_draft_observation_tdd.rs");
  for (const name of ["defaults", "fixture", "behavior", "faults", "lifecycle", "concurrency"]) assert.match(target, new RegExp(`session_catalog_draft_observation/${name}\\.rs`));
  assert.equal([...await read(`${integration}defaults.rs`).then(code => code.matchAll(/#\[tokio::test\]/g))].length, 4);
  const units = await read(`${production}observation_tests.rs`);
  assert.equal([...units.matchAll(/#\[tokio::test\]/g)].length, 6);
  assert.match(await read(`${production}mod.rs`), /#\[cfg\(test\)\]\s*mod observation_tests;/);
  const fixture = await read(`${integration}fixture.rs`);
  assert.match(fixture, /pub use observation_fixture::snapshot/);
  assert.match(fixture, /all source\/resource tables and file bytes\/modes remain unchanged/);
  assert.match(fixture, /assert_eq!\(progress\(attempt\), state/);
  const snapshot = await read("engine/tests/session_task_draft_observation/fixture.rs");
  assert.match(snapshot, /for schema in \[&f\.auth\.base\.schema, &f\.artifact_schema, &f\.task_schema\]/);
  for (const pattern of [/row_to_json/, /entry\.file_name/, /metadata\.mode\(\)/, /Sha256::digest\(fs::read/, /root\.dev\(\), root\.ino\(\), root\.mode\(\)/]) assert.match(snapshot, pattern);
  const faults = await read(`${integration}faults.rs`);
  assert.match(faults, /"binding-pair"/);
  assert.match(faults, /TaskStoreError::InvalidRecord/);
  assert.match(faults, /row\.get::<bool, _>\("is_called"\)/);
  assert.match(faults, /metadata injection must change actual stored full definition/);
  for (const [path, source] of [...await sources(integration, /\.rs$/), ["engine/tests/session_catalog_draft_observation_tdd.rs", target], [`${production}observation.rs`, await read(`${production}observation.rs`)], [`${production}observation_tests.rs`, units]]) assert.ok(source.split("\n").length - 1 <= 600, path);
});

test("all catalogue observation PostgreSQL cases run exactly in the existing disposable CI fixture", async () => {
  const cases = [];
  for (const [path, source] of await sources(integration, /\.rs$/)) {
    const prefix = path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_catalog_observation_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 8);
  const runner = await read("scripts/test-session-catalog-draft-observation.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_catalog_observation_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_catalog_draft_observation_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-session-catalog-draft-publication\.sh && bash scripts\/test-session-catalog-draft-observation\.sh/);
  assert.ok(workflow.split("\n").length - 1 <= 600);
  assert.ok((await read("scripts/tests/postgresCi.test.mjs")).split("\n").length - 1 <= 600);
});
