import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const dense = source => source.replace(/\/\/[^\n]*/g, "").replace(/\s+/g, "");
const ordered = (source, values) => {
  let offset = -1;
  for (const value of values) {
    offset = source.indexOf(value, offset + 1);
    assert.ok(offset >= 0, `present and ordered: ${value}`);
  }
};

// These are wiring inventories, not proof of database, browser or deployment acceptance.
test("Task-bound text HTTP delegates once after strict route credentials capacity body and baseline gates", async () => {
  const base = "engine/src/platform_http/task_text_artifact_publish";
  const module = dense(await read(`${base}/mod.rs`));
  assert.ok(module.includes('"/platform/v1/tasks/{task_id}/text-artifacts"'));
  assert.equal([...module.matchAll(/\.route\(/g)].length, 1);
  assert.ok(module.includes("Self::with_capacity(connector,4)"));
  assert.ok(module.includes("connector.create_session_task_text_artifact(token,expected,artifact,revision,text)"));
  const handler = dense(await read(`${base}/handlers.rs`));
  ordered(handler, ["endpoint(&request)", "security::credentials(", "admission::acquire(", "handle(state,id,token,request).await",
    "admission::response(response,permit)"]);
  ordered(handler, ["security::bounded_request_bytes(request,false,text::MAX_WIRE_BYTES)", "contract::parse(&bytes)",
    "request.into_command(id,state.scope)", "state.backend.create(", ".map_err(publication_error)",
    "StatusCode::CREATED", "text::TextResponse::from(committed)"]);
  assert.equal([...handler.matchAll(/state\.backend\.create\(/g)].length, 1);
  assert.doesNotMatch(handler, /timeout\(|retry|\.get_session|\.replace_session|\.create_session_content/);
  const contract = await read(`${base}/contract.rs`);
  assert.deepEqual([...contract.split("struct Create {")[1].split("\n}")[0].matchAll(/^\s*(\w+):/gm)].map(m => m[1]),
    ["schema_version", "expected", "artifact_id", "revision_id", "text"]);
  for (const field of ["expected: MapOnly<Expected>", "task: MapOnly<Task>", "definition: ()", "status: String",
    "reference: MapOnly<Reference>", "owner: MapOnly<PrincipalRef>", "input_refs: Vec<MapOnly<Pin>>",
    "workspace: MapOnly<WorkspaceRef>", "validate_task_snapshot", "validate_text_bytes"])
    assert.ok(contract.includes(field), field);
  assert.doesNotMatch(contract, /serde_json::Value|from_value|TaskSnapshot.*Deserialize/);
});

test("Task-bound HTTP keeps its standalone builder and publication-only conflicts do not change the old Task projection", async () => {
  const exports = await read("engine/src/platform_http/mod.rs");
  assert.match(exports, /build_task_text_artifact_publish_router, TaskTextArtifactPublishHttpState/);
  for (const path of ["engine/src/platform_http/text_artifact_publish/mod.rs",
    "engine/src/bin/cyanrex-platform/session_ui/task-edit.js", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /TaskTextArtifactPublishHttpState|build_task_text_artifact_publish_router|task_text_artifact_publish/, path);
  const projection = dense(await read("engine/src/platform_http/task_text_artifact_publish/errors.rs"));
  assert.ok(projection.includes('ArtifactStoreError::Conflict)=>{HttpError::dispatched(StatusCode::CONFLICT,"conflict")'));
  assert.ok(projection.includes('ArtifactStoreError::StaleRevision)=>{HttpError::dispatched(StatusCode::CONFLICT,"stale_revision")'));
  assert.ok(projection.includes("error=>HttpError::from(error)"));
  const old = await read("engine/src/platform_http/errors.rs");
  const task = old.split("impl From<SessionTaskError> for HttpError")[1].split("impl From<SessionArtifactError>")[0];
  assert.doesNotMatch(task, /ArtifactStoreError::Conflict|ArtifactStoreError::StaleRevision/);
});

test("six real Task-bound HTTP boundaries remain explicit ignored inventory and one CI runner selection", async () => {
  const directory = "engine/tests/platform_task_text_artifact_http", selectors = [];
  const harness = await read("engine/tests/platform_task_text_artifact_http_tdd.rs");
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_text_artifact_http_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_text_artifact_http_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason, name] of ignored) {
      assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
      assert.ok(harness.includes(`platform_task_text_artifact_http/${file}`));
      selectors.push(`${file.slice(0, -3)}::${name}`);
    }
  }
  assert.equal(selectors.length, 6);
  const runner = await read("scripts/test-platform-task-text-artifact-http.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_text_artifact_http_\w+)\b/gm)].map(m => m[1]);
  assert.equal(new Set(selected).size, 6);
  assert.deepEqual(selected.sort(), selectors.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_task_text_artifact_http_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.equal([...step.matchAll(/bash scripts\/test-platform-task-text-artifact-http\.sh/g)].length, 1);
  assert.ok(workflow.trimEnd().split("\n").length <= 600);
});

test("Task-bound HTTP guides are bilingual mirrored and distinguish publication from a Task save", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const path = `docs/${locale}/platform-text-artifact-connection.md`, guide = await read(path);
    for (const text of ["TaskTextArtifactPublishHttpState", "build_task_text_artifact_publish_router",
      "/platform/v1/tasks/{task_id}/text-artifacts", "test-platform-task-text-artifact-http.sh"])
      assert.ok(guide.includes(text), `${locale}: ${text}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-text-artifact-connection.md`));
    const security = await read(`docs/${locale}/security.md`);
    assert.ok(security.includes("TaskTextArtifactPublishHttpState"));
    assert.equal(security, await read(`frontend/public/course/${locale}/security.md`));
    await assertDocumentationReference(`docs/${locale}/security.md`, path);
  }
});
