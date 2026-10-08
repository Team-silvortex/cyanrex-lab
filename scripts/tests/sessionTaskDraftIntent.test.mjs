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

test("immutable draft intent registration stays outside live routes and browser or SDK saving", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  const surface = /\b(?:register_intent|get_session_draft_intent|DraftIntentJournal|SessionDraftIntentWorkspace|SessionDraftPublicationIntent|SessionDraftIntentError)\b/;
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), surface, path);
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), surface);
});

test("intent registration uses the original untouched attempt and shared final Session transaction", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/intent.rs"));
  assert.match(code, /impl SessionTaskDraftPublication\s*\{/);
  assert.match(code, /pub async fn register_intent\s*\(\s*&self\s*,/);
  assert.match(code, /impl DurableAuthSource\s*\{/);
  assert.match(code, /pub async fn get_session_draft_intent\s*\(\s*&self\s*,/);
  for (const pattern of [/SessionDraftPublicationState::Ready/, /self\.confirmed\.is_empty\(\)/,
    /self\.completed\.is_some\(\)/, /fingerprint\(token\)\s*!=\s*self\.fingerprint/,
    /self\s*\.\s*checkpoint\(\)/, /self\s*\.\s*source\s*\./, /begin_private_work\s*\(/,
    /\.finish\s*\(/, /\.commit\(\)\s*\.await/, /SET LOCAL synchronous_commit\s*=\s*'on'/,
    /\bLegacyAccountId\b/, /\bPrincipalRef\b/]) assert.match(code, pattern);
  const commits = [...code.matchAll(/\.commit\(\)\s*\.await/g)].length;
  const guardedReturns = [...code.matchAll(/\.finish\([\s\S]*?\.await[^;]*;\s*\w+\.commit\(\)\s*\.await[^;]*;\s*Ok\(/g)].length;
  assert.ok(commits > 0);
  assert.equal(guardedReturns, commits, "both read and registration return only after final Session checks and COMMIT");
  const registration = code.slice(code.indexOf("pub async fn register_intent("), code.indexOf("impl DurableAuthSource"));
  const reading = code.slice(code.indexOf("pub async fn get_session_draft_intent("));
  assert.equal([...registration.matchAll(/SET LOCAL synchronous_commit\s*=\s*'on'/g)].length, 2);
  assert.match(registration, /insert_in_transaction[\s\S]*SET LOCAL synchronous_commit\s*=\s*'on'[\s\S]*context\.finish/,
    "journal triggers cannot silently relax the requested commit mode; reassert before final Session checks");
  assert.match(reading, /SET LOCAL synchronous_commit\s*=\s*'on'[\s\S]*begin_private_work/);
  for (const [, signature] of code.matchAll(/pub\s+(?:async\s+)?fn\s+\w+\s*\(([^)]*)\)/g)) {
    assert.doesNotMatch(signature, /\b(?:SessionDraftPublicationCheckpoint|PrincipalRef|LegacyAccountId)\b/,
      "public entry points cannot import reported metadata or caller-selected ownership");
  }
  assert.doesNotMatch(code, /\b(?:spawn|spawn_blocking|install_empty_namespace|install_empty_schema|validate_session|create_session_artifact|create_session_content_task|read_session_artifact|get_session_content_task|replace_session_content_task|revise_session_artifact|ArtifactStore|TaskContentStore)\b/);
  assert.doesNotMatch(code, /&mut self|\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/);
  assert.doesNotMatch(code, /pub\s+(?:async\s+)?fn\s+(?:restore|resume|retry|advance|accept_receipt|into_attempt)\b/);
  const loaded = code.match(/pub struct SessionDraftPublicationIntent\s*\{([^}]*)\}/)?.[1];
  assert.ok(loaded, "loaded intent has an opaque record type");
  assert.doesNotMatch(loaded, /\bpub\b/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionDraftPublicationIntent\b/);

  const journal = (await sources("engine/src/services/draft_intent_journal", /\.rs$/))
    .filter(([path]) => !/_tests\.rs$|\/tests(?:\/|\.rs$)/.test(path))
    .map(([, source]) => codeOnly(source)).join("\n");
  const publicMethods = [...journal.matchAll(/\bpub\s+(?:async\s+)?fn\s+(\w+)\s*\(/g)].map(([, name]) => name);
  assert.deepEqual(publicMethods.sort(), ["install_empty_dispatch_namespace", "install_empty_namespace", "new"],
    "journal mutation and owner-only reads are not public adapters");
  for (const name of ["source_namespace", "task_namespace", "artifact_namespace"]) {
    assert.match(journal, new RegExp(`CASE WHEN octet_length\\(${name}\\) <= 63 THEN ${name} ELSE NULL END AS ${name}`),
      `${name} must be bounded before copying it from SQL`);
  }
  assert.match(journal, /try_get::<Option<String>/);
  for (const pattern of [/IntentRelations/, /relations\.verify\(/, /contype='p'/,
    /attnotnull/, /atttypid/, /condeferrable/, /indisvalid/]) assert.match(journal, pattern);
  const original = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/mod.rs"));
  const advance = original.slice(original.indexOf("pub async fn advance("), original.indexOf("#[cfg(test)]"));
  assert.ok(advance.includes("create_session_artifact") && advance.includes("create_session_content_task"));
  assert.doesNotMatch(advance, /\b(?:register_intent|intent|DraftIntentJournal|SessionDraftIntentWorkspace)\b/,
    "this independent registration feature does not silently gate the original dispatcher");
});

test("draft intent PostgreSQL inventory is exact and runs in the existing disposable CI fixture", async () => {
  const target = "engine/tests/session_task_draft_intent_tdd.rs";
  const cases = [];
  for (const [path, source] of [[target, await read(target)], ...await sources("engine/tests/session_task_draft_intent", /\.rs$/)]) {
    const prefix = path === target ? "" : path.split("/").at(-1).replace(/\.rs$/, "") + "::";
    for (const [, name] of source.matchAll(/async fn (postgres_draft_intent_\w+)\(/g)) cases.push(prefix + name);
  }
  assert.equal(cases.length, 12, "new PostgreSQL cases require deliberate CI inclusion");
  const runner = await read("scripts/test-session-task-draft-intent.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_draft_intent_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test session_task_draft_intent_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b/, "an ambient deployment URL cannot select the fixture");
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run Session draft publication regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-session-task-draft-publication\.sh.*&& bash scripts\/test-session-task-draft-intent\.sh/);
});
