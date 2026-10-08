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

test("journal inspection remains private metadata without public routes or automatic recovery", async () => {
  const surface = /\b(?:SessionDraftJournalInspection|inspect_session_draft_journal)\b/;
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), surface, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), surface);
});

test("journal inspection exposes opaque recorded progress only after current authorization and commit", async () => {
  const source = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/journal_inspection.rs"));
  const fields = source.match(/pub struct SessionDraftJournalInspection\s*\{([^}]*)\}/)?.[1];
  assert.ok(fields);
  assert.doesNotMatch(fields, /\bpub\s+(?!\()/);
  assert.doesNotMatch(source, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionDraftJournalInspection/);
  for (const getter of ["intent", "recorded_committed_step_count", "unknown_step", "all_steps_recorded_committed"]) {
    assert.match(source, new RegExp(`pub fn ${getter}\\(&self\\)`));
  }
  assert.doesNotMatch(source, /pub\s+(?:async\s+)?fn\s+(?:nonce|restore|resume|retry|advance|into_journaled|into_attempt|accept_receipt)\b/);
  assert.match(source, /valid_token\(token\)/);
  assert.match(source, /bounded\(async[\s\S]*transaction\(\)\.await\?[\s\S]*begin_private_work[\s\S]*\.journal[\s\S]*context\.finish[\s\S]*tx\.commit\(\)\.await\?[\s\S]*Ok\(/);
  assert.doesNotMatch(source, /\b(?:spawn|spawn_blocking|validate_session|read_session_artifact|get_session_content_task|create_session_artifact|create_session_content_task|register_intent|install_empty_namespace|install_empty_dispatch_namespace|reserve_dispatch|execute_dispatch)\b/);
  assert.doesNotMatch(source, /(?:INSERT INTO|UPDATE |DELETE FROM|CREATE TABLE)|\b(?:std::fs|tokio::fs|ArtifactContent)\b/);
  const journal = codeOnly(await read("engine/src/services/draft_intent_journal/inspection.rs"));
  assert.match(journal, /pub\(crate\) async fn inspect_in_transaction/);
  assert.match(journal, /inspect_in_transaction[\s\S]*capture_in_transaction\(tx, task, owner, account_id\)[\s\S]*let JournalReadSnapshot \{ record, steps, \.\. \} = snapshot;[\s\S]*Ok\(Some\(\(record, steps\)\)\)/);
  assert.match(journal, /require_dispatch\(\)\?[\s\S]*read_in_transaction\(tx, task, owner, account_id\)/);
  assert.match(journal, /let Some\(record\) = record else[\s\S]*relations\.verify[\s\S]*return Ok\(None\)[\s\S]*dispatch::read_steps/);
  assert.match(journal, /verify_records\(tx, &record, last, &relations, &steps\)[\s\S]*Ok\(Some\(JournalReadSnapshot\s*\{\s*record,\s*steps,\s*relations,/);
  assert.doesNotMatch(journal, /(?:INSERT INTO|UPDATE |DELETE FROM|CREATE TABLE)|reserve_step_in_transaction|begin_step_in_transaction|create_in_transaction|create_content_task_in_transaction/);
  const original = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/mod.rs"));
  assert.match(original, /pub async fn advance\(/);
  assert.doesNotMatch(original.slice(original.indexOf("pub async fn advance(")), /\b(?:inspect_session_draft_journal|journal_inspection)\b/,
    "the separate C2-V reader must not become an implicit dependency of original C2-P dispatch");
});

test("journal inspection PostgreSQL inventory is exact and shares only the disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_journal_inspection_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_journal_inspection", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_journal_inspection_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 10, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-journal-inspection.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_journal_inspection_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_journal_inspection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b/);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh.*&& bash scripts\/test-session-task-draft-journal-inspection\.sh/);
});
