import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
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

test("content index bounds UUID pages and validates lookahead through unchanged strict readers", async () => {
  const code = codeOnly(await read("engine/src/services/task_store/content_index.rs"));
  for (const pattern of [/limit\.unwrap_or\(20\)/, /1\.\.=32/, /WHERE owner_id = \$1/, /task_id > \$2/, /ORDER BY task_id LIMIT \$3/, /\(limit \+ 1\) as i64/, /store\.transaction\(true\)/]) assert.match(code, pattern);
  assert.match(code, /Some\(_\) => Some\(definitions\.ok_or\(TaskStoreError::InvalidRecord\)\?\)/);
  assert.match(code, /content::read_content_record\(tx, scope, selected_definitions, reference, owner, lock\)/);
  assert.equal([...code.matchAll(/WHERE FALSE/g)].length, 2, "empty pages still probe required Task/outbox columns without reading rows");
  assert.ok(code.indexOf("check_schema_in_transaction(tx, scope, TASK_CONTENT_STORAGE_VERSION, lock).await?") < code.indexOf("WHERE FALSE"));
  assert.ok(code.lastIndexOf("WHERE FALSE") < code.indexOf("WHERE owner_id = $1"));
  assert.match(code, /snapshot\.task != selected/);
  assert.ok(code.indexOf("content::read_content_record(") < code.indexOf("if entries.len() < limit"));
  assert.match(code, /entries\.last\(\)\.map\(\|entry\| entry\.reference\)/);
  assert.match(code, /pub struct TaskContentIndexEntry[\s\S]*?pub payload_items: usize,/);
  assert.doesNotMatch(code, /pub (?:async )?fn (?:create|replace|transition|install|delete|into_)|Serialize|Deserialize|TaskProvider|ArtifactStore|std::(?:fs|env)|INSERT INTO|UPDATE collaboration|DELETE FROM/);
  assert.match(await read("engine/src/services/task_store/content.rs"), /catalog\.get\(&definition\.reference\) == Some\(definition\)/);
  const record = codeOnly(await read("engine/src/services/task_store/content_records.rs"));
  assert.match(record, /validate_content_record\(scope, definitions, &snapshot\)/);
  assert.match(record, /recorded != snapshot/);
  assert.match(record, /event_type\(&snapshot\.task\)/);
});

test("Session index derives owner and final authority inside one source-owned transaction", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content_index.rs"));
  assert.ok(code.indexOf("PreparedTaskIndexRead::validate(self.authority_id, token, workspace, after, limit)?") < code.indexOf("self.transaction().await?"));
  assert.equal([...code.matchAll(/self\.transaction\(\)/g)].length, 1);
  assert.match(code, /PreparedTaskIndexRead::prepare\(/);
  assert.ok(code.indexOf("prepared.finish(&mut tx).await?") < code.indexOf("tx.commit().await?"));
  assert.ok(code.indexOf("tx.commit().await?") < code.indexOf("Ok(pending)"));
  assert.doesNotMatch(code, /pub owner:|Serialize|Deserialize|Artifact|\.list\(|install|std::(?:fs|env)|spawn|Role::|teacher/i);
  const prepared = codeOnly(await read("engine/src/services/auth_service/durable_source/task_index_transaction.rs"));
  assert.match(prepared, /if !valid_token\(token\)/);
  assert.match(prepared, /validate_index_page_request\(scope, after, limit\)\?/);
  assert.match(prepared, /scope\.authority_id != authority_id/);
  assert.match(prepared, /PrivateWorkContext::begin_index_for\(authority_id, tx, token, namespace, scope\)/);
  assert.match(prepared, /read_content_index_in_transaction\(tx, scope, definitions, context\.owner, after, limit\)/);
  assert.match(prepared, /context\.finish_retained_for\(authority_id, tx\)\.await\?/);
  assert.match(prepared, /\.finish_from_source_for\(self\.authority_id, tx\)/);
  const guard = codeOnly(await read("engine/src/services/auth_service/durable_source/private_work/authority.rs"));
  assert.match(guard, /self\.recheck_session_for\(authority_id, tx\)\.await/);
  assert.match(guard, /session_transaction::record\(authority_id, tx, &self\.digest\)/);
});

test("metadata index is not mounted through the live application SDK or browser surfaces", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"].map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["sdk-js/src", /\.[cm]?[jt]s$/], ["frontend/src", /\.[cm]?[jt]sx?$/], ["frontend/pages", /\.[cm]?[jt]sx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), /TaskContentIndex(?:Entry|Page|Store)|SessionTaskContentIndexWorkspace|list_session_content_tasks/, path);
});

test("all twelve SQL index cases have an exact disposable runner and existing CI fixture", async () => {
  const expected = [];
  for (const folder of ["engine/tests/task_content_index", "engine/tests/session_task_content_index"]) {
    for (const [path, source] of await sources(folder, /\.rs$/)) {
      const prefix = path.split("/").at(-1).replace(/\.rs$/, "") + "::";
      for (const [, name] of source.matchAll(/async fn (postgres_(?:session_)?content_index_\w+)\(/g)) expected.push(prefix + name);
    }
  }
  assert.equal(expected.length, 12);
  const runner = await read("scripts/test-task-content-index.sh");
  const listed = [...runner.matchAll(/^  (\w+::postgres_(?:session_)?content_index_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), expected.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /run_content_index_case task_content_index_tdd/, /run_content_index_case session_task_content_index_tdd/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /test-session-task-content\.sh && bash scripts\/test-task-content-index\.sh && bash scripts\/test-platform-task-content-http\.sh/);
});
