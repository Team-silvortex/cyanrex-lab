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

test("checkpoint inspection remains explicit internal reading without public recovery or saving", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  const surface = /\b(?:inspect_task_draft_checkpoint|SessionDraftCheckpointObservation|SessionDraftCheckpointObservedOutcome|SessionDraftCheckpointInspectionError)\b/;
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), surface, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), surface);
});

test("checkpoint inspection uses one authorized reader and does not revive the old attempt", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/inspection.rs"));
  assert.match(code, /impl DurableAuthSource\s*\{/);
  assert.match(code, /pub async fn inspect_task_draft_checkpoint\s*\(\s*&self\s*,\s*token:\s*&str/);
  for (const command of ["read_session_artifact", "get_session_content_task"]) {
    assert.equal([...code.matchAll(new RegExp(`\\b${command}\\s*\\(`, "g"))].length, 1, `${command}: one dispatch site`);
  }
  for (const boundary of ["publication_scope", "authority_id", "reported_unconfirmed_step", "NoUnconfirmedStep",
    "ScopeMismatch", "validate_text_payload_content", "MatchesCheckpointMetadata", "DiffersFromCheckpointMetadata", "NotVisible"]) {
    assert.match(code, new RegExp(`\\b${boundary}\\b`));
  }
  assert.doesNotMatch(code, /\b(?:sqlx|query|query_as|query_scalar|spawn|spawn_blocking|install_empty_schema|install_empty_namespace|create_session_artifact|create_session_content_task|replace_session_content_task|revise_session_artifact|await_artifact|await_task|SessionTaskDraftPublication|MatchesPlannedCreate|fingerprint)\b/);
  assert.doesNotMatch(code, /&mut self|\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(|Uuid::new/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Serialize|Deserialize)[^\]]*\)\]/);
  assert.doesNotMatch(code, /pub (?:async )?fn (?:restore|resume|retry|advance|save|load|into_attempt)\s*\(/);
  const original = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/observation.rs"));
  assert.match(original, /fingerprint\(token\) != self\.fingerprint/);
  assert.doesNotMatch(original, /inspect_task_draft_checkpoint\s*\(/);
});

test("checkpoint inspection PostgreSQL inventory is exact and included in the disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_inspection_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_inspection", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_inspection_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 12, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-inspection.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_inspection_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_inspection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh && bash scripts\/test-session-task-draft-observation\.sh && bash scripts\/test-session-task-draft-inspection\.sh/);
});
