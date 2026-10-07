import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const fixtureRoot = "engine/tests/fixtures/task-draft-publication/";
const fixtures = ["future-language.json", "multilingual.json", "named-empty.json"];
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

test("native draft publication planning stays pure without issuing identity or publishing storage", async () => {
  assert.match(await read("engine/src/services/mod.rs"), /pub mod task_draft_import\s*;/);
  const files = await sources("engine/src/services/task_draft_import", /\.rs$/);
  assert.ok(files.length > 0, "the native planner implementation must exist");
  for (const [path, source] of files) {
    const code = codeOnly(source);
    assert.doesNotMatch(code, /\b(?:sqlx|tokio|reqwest|DurableAuthSource|SessionTaskContentWorkspace|TaskContentHttpState)\b/, path);
    assert.doesNotMatch(code, /\b(?:std::)?(?:env|fs|net|process)::|\b(?:env|option_env)!\s*\(/, path);
    assert.doesNotMatch(code, /\b(?:Utc|Local|SystemTime|Instant)::now\s*\(|\b(?:new_v4|new_v7)\s*\(/, path);
    assert.doesNotMatch(code, /\b(?:async|await|unsafe)\b|\b(?:query|query_as|query_scalar)\s*\(/, path);
    assert.doesNotMatch(code, /\b(?:create_session_artifact|revise_session_artifact|create_session_content_task|replace_session_content_task)\s*\(/, path);
  }
});

test("draft planning adds no normal-app route, login, browser-save or advertised SDK operation", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs"]
    .map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["engine/src/platform_http", /\.rs$/],
    ["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /\b(?:task_draft_import|TaskDraftPublicationPlan|PlannedTextPublication)\b/, path);
  }
  const openapi = JSON.parse(await read("engine/openapi/openapi.json"));
  assert.ok(Object.keys(openapi.paths).length > 0);
  for (const path of Object.keys(openapi.paths)) assert.doesNotMatch(path, /^\/platform\//, "prepared APIs remain unadvertised");
  assert.doesNotMatch(JSON.stringify(openapi), /TaskDraftPublicationPlan|PlannedTextPublication|cyanrex\.task-draft/);
});

test("Rust and frontend tests share the exact bounded publication fixture inventory", async () => {
  assert.deepEqual((await readdir(new URL(fixtureRoot, root))).sort(), fixtures);
  const rust = await read("engine/tests/task_draft_publication_tdd.rs");
  const frontend = await read("frontend/tests/taskDraft.test.mjs");
  for (const filename of fixtures) {
    const includePath = `fixtures/task-draft-publication/${filename}`.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    assert.match(rust, new RegExp(`include_bytes!\\(\\s*"${includePath}"\\s*\\)`), `${filename}: shared Rust input`);
    assert.ok(frontend.includes(`${filename}"`), `${filename}: shared frontend contract`);
    const bytes = await readFile(new URL(`${fixtureRoot}${filename}`, root));
    assert.ok(bytes.byteLength <= 8 * 1024 * 1024);
    const draft = JSON.parse(bytes.toString("utf8"));
    assert.deepEqual(Object.keys(draft), ["format", "version", "title", "payload"]);
    assert.equal(draft.format, "cyanrex.task-draft"); assert.equal(draft.version, 1);
    assert.ok(draft.title.trim() && Buffer.byteLength(draft.title) <= 256);
    assert.ok(draft.payload.length <= 32);
    for (const item of draft.payload) {
      assert.deepEqual(Object.keys(item), ["id", "revision", "kind", "filename", "language", "text"]);
      assert.equal(item.kind, "text"); assert.ok(item.filename.length >= 1 && item.filename.length <= 128);
      assert.match(item.language, /^[a-z][a-z0-9_+.-]{0,63}$/);
      assert.ok(Number.isSafeInteger(item.revision) && item.revision >= 1);
      assert.ok(Buffer.byteLength(item.text) <= 256 * 1024);
    }
  }
});
