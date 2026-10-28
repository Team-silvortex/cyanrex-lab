import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const ordered = (source, values) => {
  let offset = -1;
  for (const value of values) {
    offset = source.indexOf(value, offset + 1);
    assert.ok(offset >= 0, `present and ordered: ${value}`);
  }
};

// These source inventories supplement, not replace, restricted-role SQL/HTTP regressions.
test("retained replacement validates input before one original connection and commits only after final guards", async () => {
  const source = code(await read("engine/src/platform_host/task_replace_connection.rs"));
  const dense = compact(source);
  assert.match(source, /storage: Arc<PlatformStorageConnector>/);
  assert.match(source, /workers: Arc<Semaphore>/);
  assert.deepEqual([...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "replace_session_content_task"]);
  assert.equal([...dense.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  ordered(dense, ["valid_token(token)", "TaskContentReplaceWorkspace::validate_manifest(self.scope(),&manifest)?",
    ".storage.connect()", 'begin_with("BEGINISOLATIONLEVELREADCOMMITTED")', "self.verify(&muttx).await?",
    "PreparedTaskContentReplace::prepare(", "workspace.verify_root().await?", ".verify_artifact_root()",
    "self.verify(&muttx).await?", "prepared.finish(&muttx).await?", "tx.commit().await.map_err(DurableAuthError::from)?"]);
  const finish = dense.indexOf("prepared.finish(&muttx).await?");
  const commit = dense.indexOf("tx.commit().await.map_err(DurableAuthError::from)?", finish);
  assert.equal(dense.slice(finish, commit), "prepared.finish(&muttx).await?;");
  assert.doesNotMatch(source, /PgPool|PoolOptions|connect_lazy|DurableAuthSource|\.validate_session\(|SET ROLE|SET TRANSACTION READ WRITE|std::(?:env|fs)|derive\([^)]*(?:Clone|Debug|Serialize)/);
});

test("replacement flushes deferred work before the complete old new byte union and expected pair recheck", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/task_replace_transaction.rs"));
  ordered(source, ["PrivateWorkContext::begin_index_for(", "get_content_in_transaction(",
    "replacement_content_checks(", "context.select_target(tx,&workspace.artifacts).await?",
    "verify_pinned_contents(", "context.select_target(tx,&workspace.task).await?",
    "replace_manual_content_in_transaction(", 'sqlx::query("SETCONSTRAINTSALLIMMEDIATE")',
    "context.select_target(tx,&workspace.artifacts).await?", "verify_pinned_contents(",
    "context.select_target(tx,&workspace.task).await?", "get_content_in_transaction(", "Some(&pending)",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(source.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  assert.doesNotMatch(source, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|create_session_artifact|revise_session_artifact|TaskProvider|Runner|std::(?:env|fs)/);
  const payload = compact(await read("engine/src/services/auth_service/durable_source/content_payload.rs"));
  for (const value of ["replacement_content_checks(", "verify_pinned_contents(", "sort_by_key(",
    "artifact_id.as_uuid()", "revision_id.as_uuid()", "validate_text_payload_content(",
    "TaskStoreError::InvalidRecord", "TaskStoreError::InvalidInput"])
    assert.ok(payload.includes(value), value);
  assert.doesNotMatch(payload, /\.dedup|HashSet|BTreeSet|PgPool|\.pool|\.begin\(|\.commit\(/);
  const old = compact(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  assert.ok(old.includes("replacement_content_checks("));
  assert.ok(old.includes("verify_pinned_contents("));
});

test("replacement HTTP delegates the original strict manifest DTO without publication or a dispatched timeout", async () => {
  const source = compact(await read("engine/src/platform_http/task_content_replace/mod.rs"));
  const handler = compact(await read("engine/src/platform_http/task_content_replace/handlers.rs"));
  assert.equal([...source.matchAll(/\.route\(/g)].length, 1);
  assert.ok(source.includes('"/platform/v1/tasks/{task_id}/content"'));
  assert.ok(source.includes("Self::with_capacity(connector,4)"));
  assert.ok(source.includes("connector.replace_session_content_task("));
  assert.ok(handler.includes("contract::Replace"));
  assert.ok(handler.includes("contract::parse(&bytes)?"));
  assert.ok(handler.includes("security::credentials("));
  assert.ok(handler.includes("admission::acquire("));
  assert.ok(handler.includes("admission::response("));
  assert.ok(handler.includes("security::request_bytes(request,false).await?"));
  assert.ok(handler.includes("StatusCode::OK"));
  assert.ok(handler.includes("contract::SnapshotResponse::from("));
  assert.ok(handler.includes("Method::PUT"));
  assert.doesNotMatch(source + handler, /SET_COOKIE|Set-Cookie|timeout\(|build_task_content_router|create_session_artifact|revise_session_artifact|StatusCode::CREATED/);
});

test("replacement adds only an independent sixth workspace lane and leaves readonly capabilities unchanged", async () => {
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.equal([...workspace.split("ifletSome(text)=state.text")[0].matchAll(/\.merge\(/g)].length, 6);
  assert.equal([...workspace.matchAll(/\.fallback\(/g)].length, 1);
  ordered(workspace, ["config::validate(", "replace_capacity,", "split_platform_task_replace_connectors(storage)",
    "SessionLoginHttpState::with_capacity("]);
  for (const value of ["with_capacities(", "with_content_capacities(", "with_create_capacities(", "with_replace_capacities(",
    "TaskContentReplaceHttpState::with_capacity(replace,replace_capacity)", "task_content_replace::routes().with_state(state.replace)"])
    assert.ok(workspace.includes(value), value);
  const pair = compact(await read("engine/src/platform_host/session_pair.rs"));
  const split = pair.slice(pair.indexOf("pub(crate)fnsplit_platform_task_replace_connectors("))
    .split("pub(crate)fnsplit_platform_task_text_publication_connectors(")[0];
  assert.ok(split.includes("PlatformTaskReplaceConnector::from_shared(storage)"));
  assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, 5);
  for (const path of ["engine/src/platform_host/task_content_connection.rs", "engine/src/platform_http/task_content_read/mod.rs",
    "engine/src/platform_http/task_create/mod.rs", "engine/src/platform_http/session_router/mod.rs"])
    assert.doesNotMatch(code(await read(path)), /PlatformTaskReplaceConnector|TaskContentReplaceHttpState|task_content_replace/, path);
  for (const path of ["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /PlatformTaskReplaceConnector|TaskContentReplaceHttpState|build_task_content_replace_router/, path);
});

test("ten exact replacement SQL cases are registered once and selected by disposable CI", async () => {
  const dir = "engine/tests/platform_task_replace_connection";
  const harness = await read("engine/tests/platform_task_replace_connection_tdd.rs");
  const cases = [];
  for (const file of await readdir(new URL(`${dir}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${dir}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_replace_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_replace_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason] of ignored) assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`platform_task_replace_connection/${file}`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
  }
  assert.equal(cases.length, 10);
  const runner = await read("scripts/test-platform-task-replace-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_replace_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, 10);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_task_replace_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-platform-task-replace-connection\.sh/);
});

test("bilingual replacement guides distinguish verified manifest edits from byte publication and browser saving", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-task-replace-connection.md`);
    for (const value of ["issues/3", "PlatformTaskReplaceConnector", "TaskContentReplaceHttpState", "with_replace_capacities",
      "expected_revision", "READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE", "COMMIT", "unconfirmed", "READ ONLY",
      "/platform/v1/tasks/{task_id}/content", "1–16", "SDK/OpenAPI", "CYANREX_TEST_DATABASE_URL",
      "platform-task-create-connection.md", "platform-task-content-connection.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-task-replace-connection.md`));
    for (const file of ["README.md", "architecture.md", "architecture-map.md", "platform-network.md", "testing-guide.md",
      "platform-loopback-host.md", "platform-loopback-tls-host.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-task-replace-connection.md`);
  }
});
