import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const base = "engine/src/services/auth_service/durable_source/";
const production = `${base}catalog_draft_publication/`;
const integration = "engine/tests/session_catalog_draft_publication/";
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

test("catalogue publication has its own typed attempt and freezes exact metadata before IDs", async () => {
  const code = codeOnly(await read(`${production}mod.rs`));
  assert.match(code, /pub struct SessionCatalogTaskDraftPublication\s*\{/);
  assert.match(code, /workspace: SessionCatalogTaskContentWorkspace/);
  assert.match(code, /definition: TaskDefinition/);
  assert.match(code, /pub fn definition\(&self\) -> &TaskDefinition/);
  const prepare = code.slice(code.indexOf("pub fn prepare_catalog_task_draft_publication"), code.indexOf("impl SessionCatalogTaskDraftPublication"));
  assert.match(prepare, /!valid_token\(token\) \|\| scope\.authority_id != self\.authority_id/);
  assert.match(prepare, /publication_definition\(reference\)[\s\S]*\.cloned\(\)[\s\S]*InvalidPreparation/);
  assert.ok(prepare.indexOf("publication_definition(reference)") < prepare.indexOf("uuid::Uuid::new_v4()"));
  assert.match(code, /cyanrex\.catalog-task-draft-publication\.session\.v1/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionCatalogTaskDraftPublication/);
  assert.doesNotMatch(code, /\b(?:SessionTaskDraftPublication|SessionDraftPublicationCheckpoint|SessionDraftPublicationJournal|Deref)\b/);
  assert.doesNotMatch(code, /pub\s+(?:async\s+)?fn\s+(?:restore|resume|reset|retry|observe|checkpoint|into_journaled|accept_receipt|cleanup)\b/);
  assert.match(await read(`${base}mod.rs`), /pub use catalog_draft_publication::SessionCatalogTaskDraftPublication/);
});

test("catalogue publication dispatches only existing Session creates and saves exact receipts before further awaits", async () => {
  const module = codeOnly(await read(`${production}mod.rs`));
  const confirmation = codeOnly(await read(`${production}confirmation.rs`));
  const code = module + confirmation;
  for (const command of ["create_session_artifact", "create_session_catalog_content_task"]) {
    assert.equal([...code.matchAll(new RegExp(`\\b${command}\\s*\\(`, "g"))].length, 1, command);
  }
  assert.doesNotMatch(code, /\b(?:sqlx|query|query_as|query_scalar|spawn|spawn_blocking|install_empty_namespace|create_session_content_task|replace_session_catalog_content_task|revise_session_artifact)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/);
  const artifact = confirmation.slice(confirmation.indexOf("async fn await_artifact"), confirmation.indexOf("fn artifact_matches_planned_create"));
  const task = confirmation.slice(confirmation.indexOf("async fn await_task"), confirmation.indexOf("fn task_matches_planned_create"));
  for (const part of [artifact, task]) {
    assert.ok(part.indexOf("SessionDraftPublicationState::Unconfirmed") < part.indexOf("command.await?"));
    assert.equal([...part.matchAll(/\.await\b/g)].length, 1);
  }
  assert.match(artifact, /self\.confirmed\.push\(revision\);\s*self\.state = SessionDraftPublicationState::Ready/);
  assert.match(task, /self\.completed = Some\(snapshot\);\s*self\.state = SessionDraftPublicationState::Complete/);
  assert.match(confirmation, /snapshot\.task\.definition\.as_ref\(\) == Some\(&self\.definition\)/);
  assert.match(confirmation, /snapshot\.task\.status == TaskStatus::Draft/);
  assert.match(confirmation, /validate_task_snapshot\(&snapshot\.task\)/);
});

test("only two bounded pure consistency helpers are shared while the manual matcher remains None-only", async () => {
  const code = codeOnly(await read(`${base}draft_publication_checks.rs`));
  const helperNames = [...code.matchAll(/pub\(in crate::services::auth_service::durable_source\) fn (\w+)/g)].map(([, name]) => name);
  assert.deepEqual(helperNames, ["artifact_matches_planned_create", "publication_manifest"]);
  assert.doesNotMatch(code, /\b(?:async|sqlx|query|spawn|DurableAuthSource|SessionTaskDraftPublication|SessionCatalogTaskDraftPublication|TaskDefinition|SessionCommandError)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::/);
  assert.match(code, /plan\.items\(\)\.get\(index\), allocated\.get\(index\)/);
  assert.ok(code.indexOf("confirmed.len() != plan.items().len()") < code.indexOf(".zip(plan.items())"));
  const manual = codeOnly(await read(`${base}draft_publication/confirmation.rs`));
  assert.match(manual, /snapshot\.task\.definition\.is_none\(\)/);
  assert.doesNotMatch(manual, /SessionCatalogTaskDraftPublication|TaskDefinition/);
  for (const path of [`${production}confirmation.rs`, `${base}draft_publication/confirmation.rs`]) {
    const source = codeOnly(await read(path));
    for (const helper of helperNames) assert.match(source, new RegExp(`draft_publication_checks::${helper}\\(`));
  }
});

test("catalogue attempts are not wired into HTTP application SDK or legacy recovery types", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"].map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/], ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  const oldRecovery = (await sources(`${base}draft_publication`, /\.rs$/)).filter(([path]) => !/(?:tests|test_support)/.test(path));
  files.push(...oldRecovery);
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), /\b(?:SessionCatalogTaskDraftPublication|prepare_catalog_task_draft_publication|catalog_draft_publication)\b/, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), /SessionCatalogTaskDraftPublication|prepare_catalog_task_draft_publication/);
});

test("catalogue default and controlled receipt tests are deliberately mounted and remain bounded", async () => {
  const target = await read("engine/tests/session_catalog_draft_publication_tdd.rs");
  for (const module of ["defaults", "fixture", "sql_fixture", "behavior", "faults", "lifecycle", "waits"]) {
    assert.match(target, new RegExp(`session_catalog_draft_publication/${module}\\.rs`));
  }
  const defaults = await read(`${integration}defaults.rs`);
  const receipts = await read(`${production}tests.rs`);
  assert.equal([...defaults.matchAll(/#\[tokio::test\]/g)].length, 4);
  assert.equal([...receipts.matchAll(/#\[tokio::test\]/g)].length, 8);
  assert.match(await read(`${production}mod.rs`), /#\[cfg\(test\)\]\s*mod tests;/);
  assert.match(receipts, /shared_manifest_helper_rejects_short_and_extra_receipt_prefix_before_zip/);
  assert.match(receipts, /task_receipt_requires_complete_frozen_definition_not_only_reference_or_manual_mode/);
  assert.match(receipts, /dropped_polled_artifact_and_task_futures_keep_prefix_and_stop_without_retry/);
  assert.match(await read(`${integration}fixture.rs`), /DirBuilder::new\(\)\.mode\(0o700\)/);
  for (const [path, source] of [...await sources(production, /\.rs$/), ...await sources(integration, /\.rs$/), ["engine/tests/session_catalog_draft_publication_tdd.rs", target]]) {
    assert.ok(source.split("\n").length - 1 <= 600, `${path}: ≤600 lines`);
  }
});

test("all catalogue publication PostgreSQL cases are selected exactly in the existing explicit CI fixture", async () => {
  const cases = [];
  for (const [path, source] of await sources(integration, /\.rs$/)) {
    const prefix = path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_catalog_publication_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 8, "changed SQL inventory requires deliberate runner inclusion");
  const runner = await read("scripts/test-session-catalog-draft-publication.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_catalog_publication_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_catalog_draft_publication_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh.*&& bash scripts\/test-session-catalog-draft-publication\.sh/);
  assert.ok(workflow.split("\n").length - 1 <= 600);
  assert.ok((await read("scripts/tests/postgresCi.test.mjs")).split("\n").length - 1 <= 600);
});
