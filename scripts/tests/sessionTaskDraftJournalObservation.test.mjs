import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

test("journal step observation is not a public route, browser save, SDK or recovery surface", async () => {
  const surface = /\b(?:SessionDraftJournalStepObservation|SessionDraftJournalObservedOutcome|observe_session_draft_journal_step)\b/;
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), surface, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), surface);
});

test("journal step observation retains the first full journal snapshot across one borrowed resource read", async () => {
  const base = "engine/src/services/auth_service/durable_source/";
  const source = codeOnly(await read(`${base}draft_publication/journal_observation.rs`));
  const fields = source.match(/pub struct SessionDraftJournalStepObservation\s*\{([^}]*)\}/)?.[1];
  assert.ok(fields);
  assert.doesNotMatch(fields, /\bpub\s+(?!\()|\b(?:nonce|content|bytes|text|checkpoint)\s*:/);
  assert.doesNotMatch(source, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionDraftJournalStepObservation/);
  for (const getter of ["step", "recorded_status", "result"]) assert.match(source, new RegExp(`pub fn ${getter}\\(&self\\)`));
  assert.doesNotMatch(source, /pub\s+(?:async\s+)?fn\s+(?:nonce|restore|resume|retry|advance|into_attempt|accept_receipt)\b/);
  assert.match(source, /Duration::from_secs\(10\)/);
  assert.match(source, /if ordinal > 32[\s\S]*valid_token\(token\)[\s\S]*bounded\(async/);
  assert.equal([...source.matchAll(/\.transaction\(\)/g)].length, 1);
  assert.equal([...source.matchAll(/\.capture_in_transaction\(/g)].length, 1);
  assert.match(source, /transaction\(\)\.await\?[\s\S]*begin_private_work[\s\S]*check_routes[\s\S]*capture_in_transaction\(&mut tx, reference, context\.owner, context\.account_id\(\)\)[\s\S]*check_record_routes/);
  assert.match(source, /snapshot\.steps\(\)\.get\(ordinal\)[\s\S]*read_in_transaction\(&mut tx, item\.artifact\(\), context\.owner\)[\s\S]*inspection::artifact_outcome/);
  assert.match(source, /read_content_task_in_transaction\(\s*&mut tx,\s*&mut context,[\s\S]*inspection::task_outcome/);
  assert.match(source, /NoRecordedStep\)[\s\S]*select_target\(&mut tx, &workspace\.journal_namespace\)[\s\S]*snapshot\.verify\(&workspace\.journal, &mut tx\)\.await\?;[\s\S]*context\.finish\(self, &mut tx\)\.await\?;\s*tx\.commit\(\)\.await\?;\s*pending/);
  assert.doesNotMatch(source, /\b(?:spawn|spawn_blocking|validate_session|read_session_artifact|get_session_content_task|create_session_artifact|create_session_content_task|create_in_transaction|reserve_dispatch|execute_dispatch|install_empty_namespace)\b/);
  assert.doesNotMatch(source, /(?:INSERT INTO|UPDATE |DELETE FROM|CREATE TABLE)|\b(?:std::fs|tokio::fs)\b/);
  const journal = codeOnly(await read("engine/src/services/draft_intent_journal/inspection.rs"));
  const snapshotFields = journal.match(/pub\(crate\) struct JournalReadSnapshot\s*\{([^}]*)\}/)?.[1];
  assert.ok(snapshotFields);
  assert.doesNotMatch(snapshotFields, /\bpub\b/);
  for (const field of ["record: DraftIntentRecord", "steps: Vec<DraftDispatchRecord>", "relations: IntentRelations"]) assert.ok(snapshotFields.includes(field));
  const verify = journal.slice(journal.indexOf("pub(crate) async fn verify("), journal.indexOf("impl DraftIntentJournal"));
  assert.match(verify, /verify_records\(tx, &self\.record, last, &self\.relations, &self\.steps\)/);
  assert.doesNotMatch(verify, /check_schema|capture_in_transaction|read_in_transaction/,
    "revisiting must verify the FIRST captured relation identities and complete records, not recapture them");
  const capture = journal.slice(journal.indexOf("pub(crate) async fn capture_in_transaction("));
  assert.match(capture, /let relations = self\.check_schema\(tx\)\.await\?[\s\S]*require_dispatch[\s\S]*read_in_transaction[\s\S]*dispatch::read_steps[\s\S]*verify_records\(tx, &record, last, &relations, &steps\)[\s\S]*JournalReadSnapshot\s*\{\s*record,\s*steps,\s*relations,/);
  const content = codeOnly(await read(`${base}task_commands/content.rs`));
  const helper = content.slice(content.indexOf("async fn read_content_task_in_transaction("), content.indexOf("async fn execute_content_task("));
  assert.match(helper, /execute_content_task\(tx, context, workspace, id, ContentOperation::Read\)/);
  assert.doesNotMatch(helper, /\.commit\(|begin_private_work|\.transaction\(|\.finish\(/);
  for (const path of ["mod.rs", "dispatch.rs"]) {
    const original = codeOnly(await read(`${base}draft_publication/${path}`));
    assert.doesNotMatch(original.slice(original.indexOf("pub async fn advance(")), /\b(?:observe_session_draft_journal_step|journal_observation)\b/,
      `${path}: observation must not become an implicit dispatch dependency`);
  }
});

test("journal step observation PostgreSQL inventory is exact and requires the disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_journal_observation_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_journal_observation", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_journal_observation_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 12, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-journal-observation.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_journal_observation_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_journal_observation_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b/);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh.*&& bash scripts\/test-session-task-draft-journal-observation\.sh/);
});
