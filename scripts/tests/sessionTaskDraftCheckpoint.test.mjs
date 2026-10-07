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

test("draft checkpoint stays internal without advertising persistence or browser recovery", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /\b(?:SessionDraftPublicationCheckpoint|SessionDraftCheckpointState|MAX_SESSION_DRAFT_CHECKPOINT_BYTES)\b/, path);
  }
  assert.doesNotMatch(await read("engine/openapi/openapi.json"), /SessionDraftPublicationCheckpoint|SessionDraftCheckpointState|cyanrex\.task-draft-checkpoint/);
});

test("checkpoint codec admits bounded named records without erasing duplicate fields", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/checkpoint.rs"));
  assert.match(code, /MAX_SESSION_DRAFT_CHECKPOINT_BYTES\s*:\s*usize\s*=\s*64\s*\*\s*1024/);
  assert.match(code, /pub fn parse_json\s*\(\s*bytes:\s*&\[u8\]/);
  assert.match(code, /pub fn to_json\s*\(\s*&self\s*\)/);
  assert.match(code, /deserialize_map\s*\(/);
  assert.match(code, /deny_unknown_fields/);
  assert.match(code, /MapOnly<RawTaskRef>/);
  assert.match(code, /MapOnly<WorkspaceRef>/);
  assert.doesNotMatch(code, /serde_json::(?:from_value|Value|json)|\bserde_json::value::/);
  for (const name of ["reported_state", "reported_confirmed_artifact_count", "reported_unconfirmed_step"]) {
    assert.match(code, new RegExp(`pub fn ${name}\\s*\\(`));
  }
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Clone|Debug|Serialize|Deserialize)[^\]]*\)\]\s*pub struct SessionDraftPublicationCheckpoint\b/);
});

test("checkpoint export and parse cannot revive an attempt, dispatch commands or choose storage", async () => {
  const code = codeOnly(await read("engine/src/services/auth_service/durable_source/draft_publication/checkpoint.rs"));
  assert.match(code, /pub fn checkpoint\s*\(\s*&self\s*\)/);
  assert.doesNotMatch(code, /&mut self|pub async fn/);
  assert.doesNotMatch(code, /\b(?:sqlx|query|query_as|query_scalar|spawn|spawn_blocking|DurableAuthSource|SessionTaskContentWorkspace|SessionArtifactWorkspace)\b/);
  assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(|\b(?:Utc|SystemTime)::|Uuid::new/);
  assert.doesNotMatch(code, /\b(?:fingerprint|source|namespace|credential|owner|token)\s*:/);
  assert.doesNotMatch(code, /\b(?:create_session_artifact|create_session_content_task|read_session_artifact|get_session_content_task|await_artifact|await_task)\s*\(/);
  assert.doesNotMatch(code, /pub fn (?:restore|resume|retry|advance|observe_unconfirmed|into_attempt|from_checkpoint|save|load)\s*\(/);
  const publication = (await sources("engine/src/services/auth_service/durable_source/draft_publication", /\.rs$/))
    .filter(([path]) => !/\/(?:tests|test_support)(?:\/|\.rs$)|_tests\.rs$/.test(path))
    .map(([, source]) => codeOnly(source)).join("\n");
  assert.doesNotMatch(publication, /impl(?:<[^>]*>)?\s+(?:From|TryFrom)<SessionDraftPublicationCheckpoint>\s+for\s+SessionTaskDraftPublication/);
  const target = "engine/tests/session_task_draft_checkpoint_tdd.rs";
  const tests = [[target, await read(target)], ...await sources("engine/tests/session_task_draft_checkpoint", /\.rs$/)];
  const cases = tests.flatMap(([, source]) => [...source.matchAll(/#\[(?:test|tokio::test)\]\s*(?:async\s+)?fn (checkpoint_\w+)\(/g)].map(([, name]) => name));
  assert.equal(cases.length, 14, "the default contract inventory must not silently shrink");
  assert.equal(new Set(cases).size, 14);
  for (const [path, source] of tests) assert.doesNotMatch(source, /#\[ignore(?:\]|\s*=)/, path);
  assert.match(await read("scripts/quality-gate.sh"), /cargo test --manifest-path .* --locked\n/);
});
