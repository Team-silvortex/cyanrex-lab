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

test("journaled dispatch stays internal without public, browser or SDK saving", async () => {
  const surface = /\b(?:SessionJournaledTaskDraftPublication|SessionDraftDispatchError|into_journaled|install_empty_dispatch_namespace)\b/;
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), surface, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), surface);
});

test("journaled dispatch consumes the original attempt and commits a sealed permit before resource work", async () => {
  const wrapper = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/dispatch.rs"));
  const transaction = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/dispatch_transaction.rs"));
  assert.match(wrapper, /pub fn into_journaled\(\s*self,/);
  for (const pattern of [/SessionDraftPublicationState::Ready/, /confirmed\.is_empty\(\)/, /completed\.is_some\(\)/,
    /matches_attempt\(&self\)/, /checkpoint\(\)/, /fingerprint\(token\) != self\.attempt\.fingerprint/]) assert.match(wrapper, pattern);
  assert.doesNotMatch(wrapper, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionJournaledTaskDraftPublication/);
  const fields = wrapper.match(/pub struct SessionJournaledTaskDraftPublication\s*\{([^}]*)\}/)?.[1];
  assert.ok(fields);
  assert.doesNotMatch(fields, /\bpub\s+(?!\()/);
  assert.doesNotMatch(wrapper, /pub\s+(?:async\s+)?fn\s+(?:restore|resume|retry|into_attempt|unwrap|accept_receipt)\b/);
  assert.doesNotMatch(wrapper, /\b(?:sqlx|spawn|spawn_blocking|create_session_artifact|create_session_content_task)\b/);
  const advance = wrapper.slice(wrapper.indexOf("pub async fn advance("));
  assert.match(advance, /Unconfirmed[\s\S]*reserve_dispatch\(token, ordinal\)\.await\?[\s\S]*execute_dispatch\(token, permit, operation\)\.await\?/);
  assert.doesNotMatch(advance.slice(advance.indexOf("match pending")), /\.await/,
    "acknowledged business receipts are saved without another cancellation point");
  const permit = transaction.match(/pub\(super\) struct DispatchPermit\s*\{([^}]*)\}/)?.[1];
  assert.ok(permit);
  assert.doesNotMatch(permit, /\bpub\b/);
  assert.equal([...transaction.matchAll(/Ok\(DispatchPermit\s*\{/g)].length, 1);
  const reserve = transaction.slice(transaction.indexOf("pub(super) async fn reserve_dispatch("), transaction.indexOf("pub(super) async fn execute_dispatch("));
  assert.match(reserve, /begin_private_work[\s\S]*reserve_step_in_transaction[\s\S]*context\.finish[\s\S]*tx\.commit\(\)\.await\?[\s\S]*Ok\(DispatchPermit/);
  assert.doesNotMatch(reserve, /create_in_transaction|create_content_task_in_transaction/);
  const execute = transaction.slice(transaction.indexOf("pub(super) async fn execute_dispatch("));
  for (const pattern of [/source\.transaction\(\)\.await/, /begin_private_work/,
    /context\.owner != permit\.record\.owner/, /context\.account_id\(\) != permit\.record\.account_id/,
    /context\.source_namespace\(\) != permit\.record\.source_namespace/]) assert.match(execute, pattern);
  assert.match(execute, /begin_step_in_transaction[\s\S]*create_in_transaction[\s\S]*create_content_task_in_transaction[\s\S]*verify_step_in_transaction/);
  const finalChecks = execute.slice(execute.indexOf(".verify_step_in_transaction("));
  assert.match(finalChecks, /SET LOCAL synchronous_commit = 'on'[\s\S]*context\.finish\(source, &mut tx\)\.await\?;\s*tx\.commit\(\)\.await\?;\s*Ok\(pending\)/);
  assert.doesNotMatch(finalChecks, /create_in_transaction|create_content_task_in_transaction|begin_step_in_transaction/);
  assert.equal([...transaction.matchAll(/SET LOCAL synchronous_commit = 'on'/g)].length, 4);
  assert.doesNotMatch(transaction, /\b(?:spawn|spawn_blocking|validate_session|install_empty_namespace|install_empty_dispatch_namespace|create_session_artifact|create_session_content_task)\b/);
  const journal = codeOnly(await read("engine/src/services/draft_intent_journal/dispatch.rs"));
  const verify = journal.slice(journal.indexOf("pub(crate) async fn verify_step_in_transaction("));
  assert.doesNotMatch(verify, /(?:INSERT INTO|UPDATE |DELETE FROM)/);
  for (const pattern of [/LIMIT 34/, /expected_steps/, /relations\.verify/]) assert.match(journal, pattern);
  const content = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  assert.match(content, /pub\(in crate::services::auth_service::durable_source\) async fn create_content_task_in_transaction/);
  assert.equal([...content.matchAll(/async fn execute_content_task\(/g)].length, 1,
    "journaled Task creation shares C2-M's verification rather than copying policy");
  const helper = content.slice(content.indexOf("async fn create_content_task_in_transaction("), content.indexOf("async fn execute_content_task("));
  assert.doesNotMatch(helper, /\.commit\(|begin_private_work|\.transaction\(|\.finish\(/);
});

test("journaled dispatch PostgreSQL inventory is exact and uses the disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_dispatch_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_dispatch", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_dispatch_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 14, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-dispatch.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_dispatch_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_dispatch_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b/);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh.*&& bash scripts\/test-session-task-draft-dispatch\.sh/);
});
