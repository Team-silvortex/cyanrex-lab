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

test("unconfirmed draft observation stays internal without a public recovery route or browser save", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /\b(?:observe_unconfirmed|SessionDraftPublicationObservation|SessionDraftObservedOutcome)\b/, path);
  }
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), /observe_unconfirmed|SessionDraftPublicationObservation|SessionDraftObservedOutcome/);
});

test("unconfirmed observation borrows the attempt and dispatches only existing authorized readers", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/observation.rs"));
  assert.match(code, /pub async fn observe_unconfirmed\s*\(\s*&self\s*,\s*token:\s*&str/);
  for (const command of ["read_session_artifact", "get_session_content_task"]) {
    assert.equal([...code.matchAll(new RegExp(`\\b${command}\\s*\\(`, "g"))].length, 1, `${command}: one read dispatch site`);
  }
  assert.doesNotMatch(code, /\b(?:sqlx|query|query_as|query_scalar|spawn|spawn_blocking|install_empty_schema|install_empty_namespace|create_session_artifact|create_session_content_task|replace_session_content_task|revise_session_artifact|await_artifact|await_task)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/);
  assert.doesNotMatch(code, /self\.(?:state|confirmed|completed|plan|task|allocated|fingerprint)\s*=/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Serialize|Deserialize)[^\]]*\)\]/);
  for (const outcome of ["MatchesPlannedCreate", "DiffersFromPlannedCreate", "NotVisible", "NoUnconfirmedStep", "SessionChanged"]) {
    assert.match(code, new RegExp(`\\b${outcome}\\b`));
  }
});

test("unconfirmed observation PostgreSQL cases are selected exactly in the disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_observation_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_observation", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_observation_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 12, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-observation.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_observation_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_observation_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh && bash scripts\/test-session-task-draft-observation\.sh/);
});
