import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
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

test("create-only draft publication remains internal and is not a public save or issuer", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /\b(?:SessionTaskDraftPublication|prepare_task_draft_publication|SessionDraftPublicationState|draft_publication)\b/, path);
  }
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), /SessionTaskDraftPublication|prepare_task_draft_publication/);
});

test("draft publication composes existing Session creates without detached writers or a restore surface", async () => {
  const directory = "engine/src/services/auth_service/durable_source/draft_publication";
  // C2-T/U/V/W own separate source transactions; the original C2-P path stays SQL-free.
  const files = (await sources(directory, /\.rs$/)).filter(([path]) => !/\/(?:tests|test_support)(?:\/|\.rs$)|_tests\.rs$|\/(?:intent|dispatch_transaction|journal_inspection|journal_observation)\.rs$/.test(path));
  assert.ok(files.length > 0);
  const code = files.map(([, source]) => codeOnly(source)).join("\n");
  for (const command of ["create_session_artifact", "create_session_content_task"]) {
    assert.equal([...code.matchAll(new RegExp(`\\b${command}\\s*\\(`, "g"))].length, 1, `${command}: a single dispatch site`);
  }
  assert.doesNotMatch(code, /\b(?:sqlx|query|query_as|query_scalar|spawn|spawn_blocking|install_empty_schema|install_empty_namespace|replace_session_content_task|revise_session_artifact)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/);
  assert.doesNotMatch(code, /pub\s+(?:async\s+)?fn\s+(?:restore|resume|reset|accept_receipt|retry|cleanup)\b/);
  assert.match(code, /pub struct SessionTaskDraftPublication\s*\{/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionTaskDraftPublication\b/);
});

test("draft publication PostgreSQL cases are all selected exactly and run in an explicit CI fixture", async () => {
  const cases = [];
  const target = "engine/tests/session_task_draft_publication_tdd.rs";
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_publication", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_publication_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 12, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-publication.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_publication_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_publication_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh/);
});
