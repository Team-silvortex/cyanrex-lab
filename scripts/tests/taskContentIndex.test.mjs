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
  assert.match(code, /Some\(_\) => catalog\.as_ref\(\)\.ok_or\(TaskStoreError::InvalidRecord\)\?/);
  assert.match(code, /\.read\(tx, reference, owner, lock\)/);
  assert.equal([...code.matchAll(/WHERE FALSE/g)].length, 2, "empty pages still probe required Task/outbox columns without reading rows");
  assert.ok(code.indexOf("check_schema(tx, lock).await?") < code.indexOf("WHERE FALSE"));
  assert.ok(code.lastIndexOf("WHERE FALSE") < code.indexOf("WHERE owner_id = $1"));
  assert.match(code, /snapshot\.task != selected/);
  assert.ok(code.indexOf(".read(tx, reference, owner, lock)") < code.indexOf("if entries.len() < limit"));
  assert.match(code, /entries\.last\(\)\.map\(\|entry\| entry\.reference\)/);
  assert.match(code, /pub struct TaskContentIndexEntry[\s\S]*?pub payload_items: usize,/);
  assert.doesNotMatch(code, /pub (?:async )?fn (?:create|replace|transition|install|delete|into_)|Serialize|Deserialize|TaskProvider|ArtifactStore|std::(?:fs|env)|INSERT INTO|UPDATE collaboration|DELETE FROM/);
  assert.match(await read("engine/src/services/task_store/content.rs"), /catalog\.get\(&definition\.reference\) == Some\(definition\)/);
});

test("Session index derives owner and final authority inside one source-owned transaction", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/content_index.rs"));
  assert.match(code, /if !valid_token\(token\)/);
  assert.ok(code.indexOf("validate_index_page_request(workspace.scope, after, limit)?") < code.indexOf("self.transaction().await?"));
  assert.match(code, /workspace\.scope\.authority_id != self\.authority_id/);
  assert.equal([...code.matchAll(/self\.transaction\(\)/g)].length, 1);
  assert.match(code, /begin_private_work\(&mut tx, token, &workspace\.namespace, workspace\.scope\)/);
  assert.match(code, /list_in_transaction\(&mut tx, context\.owner, after, limit\)/);
  assert.ok(code.indexOf("context.finish(self, &mut tx).await?") < code.indexOf("tx.commit().await?"));
  assert.ok(code.indexOf("tx.commit().await?") < code.indexOf("Ok(pending)"));
  assert.doesNotMatch(code, /pub owner:|Serialize|Deserialize|Artifact|\.list\(|install|std::(?:fs|env)|spawn|Role::|teacher/i);
  const guard = await read("engine/src/services/auth_service/durable_source/private_work/mod.rs");
  assert.match(guard, /recheck_command_session\(tx, &self\.digest, &self\.session\)/);
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
