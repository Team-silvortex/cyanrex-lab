import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const splitFunction = (source, name) => {
  const start = source.indexOf(`pub(crate)fn${name}(`);
  assert.ok(start >= 0, `known private split: ${name}`);
  const opening = source.indexOf("{", start);
  assert.ok(opening > start, `split body: ${name}`);
  let end = opening + 1, depth = 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    else if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0, `bounded split body: ${name}`);
  return source.slice(start, end);
};
const ordered = (source, values) => {
  let offset = -1;
  for (const value of values) {
    const next = source.indexOf(value, offset + 1);
    assert.ok(next > offset && next >= 0, `present and ordered: ${value}`);
    offset = next;
  }
};

// Wiring guards supplement genuine restricted-role, lock-wait and HTTP behavior tests.
test("retained Task index derives routing from one original binding without a pool or detached Session check", async () => {
  const source = code(await read("engine/src/platform_host/task_index_connection.rs"));
  assert.match(source, /pub struct PlatformTaskIndexConnector\s*\{\s*storage: Arc<PlatformStorageConnector>/);
  for (const value of ["pub fn new(storage: PlatformStorageConnector)",
    "Self::from_shared(Arc::new(storage))", "self.storage.config().origin()",
    "self.storage.config().scope()", "SessionTaskContentIndexWorkspace::new(",
    "self.storage.config().namespaces().task()", "PreparedTaskIndexRead::validate("])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /PgPool|PoolOptions|connect_lazy|DurableAuthSource|AuthService|\.validate_session\(|std::(?:env|fs)|inspect_|\.or_else\(|derive\([^)]*(?:Clone|Debug|Serialize)/);
  assert.deepEqual([...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "list_session_content_tasks"]);
});

test("retained index finishes original private authority after retained guards and before confirmed commit", async () => {
  const source = compact(await read("engine/src/platform_host/task_index_connection.rs"));
  assert.equal([...source.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  assert.equal([...source.matchAll(/self\.verify\(&muttx\)\.await\?/g)].length, 2);
  ordered(source, ["PreparedTaskIndexRead::validate(", ".storage.connect()",
    'begin_with("BEGINISOLATIONLEVELREADCOMMITTED")', "self.verify(&muttx).await?",
    "PreparedTaskIndexRead::prepare(", "self.verify(&muttx).await?",
    "prepared.finish(&muttx).await?", "tx.commit().await.map_err(DurableAuthError::from)?",
    "tokio::time::timeout(Duration::from_secs(10),connection.close()).await"]);
  const finish = source.indexOf("prepared.finish(&muttx).await?");
  const commit = source.indexOf("tx.commit().await.map_err(DurableAuthError::from)?", finish);
  assert.equal(source.slice(finish, commit), "prepared.finish(&muttx).await?;");
  assert.ok(source.includes("tokio::time::timeout(Duration::from_secs(10)"));
  assert.ok(source.includes(".verify_transaction(tx).await"));
  assert.doesNotMatch(source, /SETTRANSACTIONREADWRITE|retry|\.begin\(|\.commit\(\).*\.connect\(/);
});

test("prepared index shares pool-free strict reads and preserves original Source selection before final freshness", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/task_index_transaction.rs"));
  ordered(source, ["PrivateWorkContext::begin_index_for(", "read_content_index_in_transaction(",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(source.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  assert.doesNotMatch(source.split("#[cfg(test)]")[0], /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|ArtifactStore|TaskProvider|Serialize|std::(?:env|fs)/);
  const reader = compact(await read("engine/src/services/task_store/content_index.rs"));
  const borrowed = reader.slice(reader.indexOf("pub(crate)asyncfnread_content_index_in_transaction("));
  assert.doesNotMatch(borrowed, /\.pool|PgPool|connect_lazy|\.begin\(|\.commit\(|TaskContentStore::new/);
  assert.ok(borrowed.includes("content::read_content_record("));
  assert.ok(borrowed.includes("schema::check_schema_in_transaction("));
});

test("index relation pins avoid credential-wide reads while retaining fourteen original locked identities", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/source_relations.rs"));
  assert.ok(source.includes("Self::capture_mode(tx,registry,false).await"));
  assert.ok(source.includes("Self::capture_mode(tx,true,true).await"));
  const plan = source.match(/"PREPAREcyanrex_private_index_relations_v1AS([^";]+);DEALLOCATEcyanrex_private_index_relations_v1"/)?.[1];
  assert.ok(plan, "one bounded Simple Query PREPARE/DEALLOCATE batch");
  assert.deepEqual([...plan.matchAll(/ONLY(\w+?)(?=,|$)/g)].map(match => match[1]), [
    "users", "sessions", "collaboration_auth_source_schema", "collaboration_identity_schema",
    "collaboration_access_schema", "collaboration_authorities", "collaboration_workspaces",
    "collaboration_legacy_workspaces", "collaboration_principals", "collaboration_legacy_identities",
    "collaboration_memberships", "collaboration_deployment_grants", "collaboration_identity_audit",
    "collaboration_policy_audit",
  ]);
  assert.doesNotMatch(plan, /EXECUTE|password|totp|\*/);
  assert.equal([...source.matchAll(/PREPAREcyanrex_private_index_relations_v1/g)].length, 1);
  for (const value of ["oids.len()!=14", "count(DISTINCTl.relation)=14ASheld",
    "l.pid=pg_catalog.pg_backend_pid()", "l.mode='AccessShareLock'ANDl.granted",
    "Self::capture_private_index(tx).await?", "Self::capture(tx,self.registry).await?",
    "current.oids!=self.oids"])
    assert.ok(source.includes(value), value);
  const authority = compact(await read("engine/src/services/auth_service/durable_source/private_work/authority.rs"));
  assert.ok(authority.includes("RelationMode::Full=>RelationPin::capture(tx,true).await?"));
  assert.ok(authority.includes("RelationMode::Index=>RelationPin::capture_private_index(tx).await?"));
  assert.ok(authority.includes("Self::begin(authority_id,tx,token,target,scope,RelationMode::Full).await"));
  assert.ok(authority.includes("Self::begin(authority_id,tx,token,target,scope,RelationMode::Index).await"));
});

test("workspace composition adds private manifest replacement beside creation and reads without expanding Session-only composition", async () => {
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  for (const value of ["pubstructPlatformWorkspaceHttpState", "build_platform_workspace_router(",
    "session_login::routes()", "session_lifecycle::routes()", "task_index::routes()", "task_content_read::routes()", "task_create::routes()", "task_content_replace::routes()",
    "TaskContentIndexHttpState::with_platform_connector(", "config::validate(",
    "TaskContentReadHttpState::with_capacity(content,content_capacity)",
    "TaskCreateHttpState::with_capacity(create,create_capacity)",
    "TaskContentReplaceHttpState::with_capacity(replace,replace_capacity)",
    ".fallback(not_found)", ".layer(middleware::from_fn(security::response_headers))"])
    assert.ok(workspace.includes(value), value);
  assert.equal([...workspace.split("ifletSome(text)=state.text")[0].matchAll(/\.merge\(/g)].length, 6);
  assert.equal([...workspace.matchAll(/\.fallback\(/g)].length, 1);
  assert.doesNotMatch(workspace, /PgPool|DurableAuthSource|\.connect\(|\.await|build_task_content_router|build_text_artifact_router|file_distribution/);
  const pair = compact(await read("engine/src/platform_host/session_pair.rs"));
  const sessionSplit = splitFunction(pair, "split_platform_session_connectors");
  const indexSplit = splitFunction(pair, "split_platform_workspace_connectors");
  const contentSplit = splitFunction(pair, "split_platform_content_connectors");
  const createSplit = splitFunction(pair, "split_platform_task_create_connectors");
  const replaceSplit = splitFunction(pair, "split_platform_task_replace_connectors");
  for (const [split, clones, last, capabilities] of [
    [sessionSplit, 1, "PlatformSessionConnector", ["PlatformLoginConnector", "PlatformSessionConnector"]],
    [indexSplit, 2, "PlatformTaskIndexConnector", ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector"]],
    [contentSplit, 3, "PlatformTaskContentConnector", ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector"]],
    [createSplit, 4, "PlatformTaskCreateConnector", ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector", "PlatformTaskCreateConnector"]],
    [replaceSplit, 5, "PlatformTaskReplaceConnector", ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector", "PlatformTaskCreateConnector", "PlatformTaskReplaceConnector"]],
  ]) {
    assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1);
    assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, clones);
    assert.ok(split.includes(`${last}::from_shared(storage)`));
    assert.deepEqual([...split.matchAll(/(Platform\w+Connector)::from_shared\(/g)].map(match => match[1]), capabilities);
    assert.doesNotMatch(split, /inspect_|verify_|\.connect\(|std::(?:env|fs)|\.clone\(/);
  }
  const readRouter = compact(await read("engine/src/platform_http/task_content_read/mod.rs"));
  const routes = readRouter.slice(readRouter.indexOf("pub(super)fnroutes()"), readRouter.indexOf("asyncfnnot_found()"));
  assert.ok(routes.includes('route("/platform/v1/tasks/{task_id}",any(handlers::dispatch))'));
  assert.equal([...routes.matchAll(/\.route\(/g)].length, 1);
  assert.doesNotMatch(routes, /TaskCreate|task_create|TaskContentReplace|task_content_replace|\/content|\/status|\.merge\(|\.fallback\(|\.layer\(|\.with_state\(/);
  const createRouter = compact(await read("engine/src/platform_http/task_create/mod.rs"));
  const createRoutes = createRouter.slice(createRouter.indexOf("pub(super)fnroutes()"), createRouter.indexOf("asyncfnnot_found()"));
  assert.ok(createRoutes.includes('route("/platform/v1/tasks",any(handlers::dispatch))'));
  assert.equal([...createRoutes.matchAll(/\.route\(/g)].length, 1);
  assert.doesNotMatch(createRoutes, /TaskReplace|task_content_replace|\/content|\/status|\{task_id\}|\.merge\(|\.fallback\(|\.layer\(|\.with_state\(/);
  const create = compact(await read("engine/src/platform_http/task_create/handlers.rs"));
  assert.ok(create.includes("Method::POST"));
  assert.ok(create.includes("#[serde(deny_unknown_fields)]structCreateEmpty{schema_version:CoreSchemaVersion,task_id:TaskId,title:String,}"));
  assert.ok(create.includes("TaskContentManifest::new(&request.title,vec![])"));
  assert.ok(create.includes("letrequest:CreateEmpty=contract::parse(&bytes)?"));
  const session = compact(await read("engine/src/platform_http/session_router/mod.rs"));
  assert.equal([...session.matchAll(/\.merge\(/g)].length, 2);
  assert.doesNotMatch(session, /TaskIndex|TaskCreate|TaskReplace|TaskContent(?:Index|Read|Replace)?HttpState|task_index::routes|task_content|task_create|\/platform\/v1\/tasks|PlatformWorkspace/);
  const host = compact(await read("engine/src/bin/cyanrex-platform/serve_loopback/mod.rs"));
  assert.ok(host.includes("PlatformWorkspaceHttpState::with_text_publication_capacities(storage,4,4,4,4,4,4,4,)"));
  assert.ok(host.includes("build_platform_workspace_router(state)"));
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /PlatformTaskIndexConnector|PlatformWorkspaceHttpState|build_platform_workspace_router/, path);
});

test("all eight retained index SQL boundaries are wired and selected exactly in disposable CI", async () => {
  const directory = "engine/tests/platform_task_index_connection";
  const harness = await read("engine/tests/platform_task_index_connection_tdd.rs");
  const cases = [];
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const module = file.slice(0, -3);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_index_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_index_\w+)\(/g)];
    assert.equal(ignored.length, declared.length, `every actual SQL case is explicitly ignored: ${file}`);
    for (const [, reason] of ignored) assert.equal(reason, file === "host.rs"
      ? "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"
      : "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration", file);
    if (ignored.length) assert.ok(harness.includes(`#[path = "platform_task_index_connection/${file}"]\nmod ${module};`), file);
    cases.push(...ignored.map(([, , name]) => `${module}::${name}`));
    assert.ok(source.split("\n").length <= 601, file);
  }
  assert.equal(cases.length, 8, "new resource boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-task-index-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_index_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, selected.length, "runner must not repeat a case");
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/,
    /--locked --test platform_task_index_connection_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact/])
    assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-platform-task-index-connection\.sh/);
});

test("bilingual retained index guides preserve optional payload and explicit host authority boundaries", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-task-index-connection.md`);
    for (const value of ["issues/3", "PlatformTaskIndexConnector", "PlatformStorageConnector",
      "TaskContentIndexHttpState::with_platform_storage", "PlatformWorkspaceHttpState",
      "build_platform_workspace_router", "build_platform_session_router", "with_capacities",
      "serve-loopback", "serve-loopback-tls", "READ COMMITTED", "COMMIT", "READ ONLY",
      "/platform/v1/session/login", "/platform/v1/session/logout", "/platform/v1/task-index",
      "1–32", "no-store", "no-cache", "nosniff", "TOTP", "SDK/OpenAPI",
      "CYANREX_TEST_DATABASE_URL", "task-content-index-http.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en"
      ? /code, an editor, a domain definition and a Run are not prerequisites/
      : /代码、编辑器、领域定义和 Run\s*都不是前提/);
    assert.match(guide, locale === "en"
      ? /neither path connects browser saving, cross-user teaching review or Task acceptance/
      : /两者都不等于浏览器保存、跨用户教学审阅或任务验收/);
    await assertDocumentationReference(`docs/${locale}/platform-task-index-connection.md`, `docs/${locale}/platform-task-content-connection.md`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-task-index-connection.md`));
    for (const file of ["README.md", "architecture.md", "architecture-map.md", "platform-network.md",
      "testing-guide.md", "platform-loopback-host.md", "platform-loopback-tls-host.md", "platform-session-router.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-task-index-connection.md`);
  }
});
