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
    const next = source.indexOf(value, offset + 1);
    assert.ok(next >= 0 && next > offset, `present and ordered: ${value}`);
    offset = next;
  }
};
const body = (source, declaration) => {
  const start = source.indexOf(declaration);
  assert.ok(start >= 0, declaration);
  const opening = source.indexOf("{", start);
  assert.ok(opening > start, `body: ${declaration}`);
  let end = opening + 1, depth = 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    else if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0, `bounded body: ${declaration}`);
  return source.slice(start, end);
};

// Wiring inventories supplement genuine restricted-role, trigger, wait and HTTP regressions.
test("retained empty Task creation derives all routing from one original storage and validates before connecting", async () => {
  const source = code(await read("engine/src/platform_host/task_create_connection.rs"));
  const dense = compact(source);
  assert.match(source, /pub struct PlatformTaskCreateConnector\s*\{\s*storage: Arc<PlatformStorageConnector>/);
  assert.deepEqual([...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "create_session_empty_content_task"]);
  for (const value of ["Self::from_shared(Arc::new(storage))", "self.storage.config().origin()",
    "self.storage.config().scope()", "self.storage.config().namespaces().task()",
    "self.storage.config().namespaces().artifacts()"])
    assert.ok(source.includes(value), value);
  ordered(dense, ["valid_token(token)", "TaskContentManifest::new(title.to_owned(),vec![])",
    "TaskCreateWorkspace::new(", ".storage.connect()"]);
  assert.doesNotMatch(source, /PgPool|PoolOptions|connect_lazy|DurableAuthSource|AuthService|\.validate_session\(|inspect_|std::(?:env|fs)|\.or_else\(|derive\([^)]*(?:Clone|Debug|Serialize)/);
  assert.doesNotMatch(source, /ArtifactStore|TaskProvider|Runner|TaskRun|create_session_artifact|revise_session_artifact|SET TRANSACTION READ WRITE|SET ROLE/);
});

test("retained Task creation places every original guard before final fresh authority and immediate confirmed commit", async () => {
  const source = compact(await read("engine/src/platform_host/task_create_connection.rs"));
  assert.equal([...source.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  assert.equal([...source.matchAll(/self\.verify\(&muttx\)\.await\?/g)].length, 2);
  ordered(source, ['begin_with("BEGINISOLATIONLEVELREADCOMMITTED")', "self.verify(&muttx).await?",
    "PreparedTaskCreate::prepare(", ".verify_artifact_root()", "self.verify(&muttx).await?",
    "prepared.finish(&muttx).await?", "tx.commit().await.map_err(DurableAuthError::from)?",
    "tokio::time::timeout(Duration::from_secs(10),connection.close()).await"]);
  const finish = source.indexOf("prepared.finish(&muttx).await?");
  const commit = source.indexOf("tx.commit().await.map_err(DurableAuthError::from)?", finish);
  assert.equal(source.slice(finish, commit), "prepared.finish(&muttx).await?;");
  assert.ok(source.includes(".verify_transaction(tx).await"));
  assert.doesNotMatch(source, /\.begin\(|retry|rollback\(|install_|grant_|publish_/);
});

test("prepared empty creation shares pool-free policy and rechecks resources after all deferred work before final Session", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/task_create_transaction.rs"));
  ordered(source, ["valid_token(token)", "!manifest.payload().is_empty()", "authority_id!=workspace.scope.authority_id",
    "PrivateWorkContext::begin_index_for(", "get_content_in_transaction(",
    "context.select_target(tx,&workspace.artifacts).await?", "validate_artifact_namespace_in_transaction(tx,workspace.scope).await?",
    "context.select_target(tx,&workspace.task).await?", "create_manual_content_in_transaction(",
    'sqlx::query("SETCONSTRAINTSALLIMMEDIATE")',
    "context.select_target(tx,&workspace.artifacts).await?", "validate_artifact_namespace_in_transaction(tx,workspace.scope).await?",
    "context.select_target(tx,&workspace.task).await?", "get_content_in_transaction(", "Some(&pending)",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(source.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  assert.doesNotMatch(source, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|TaskProvider|Runner|TaskRun|ArtifactStore|std::(?:env|fs)|Serialize|install_|publish_/);
  const authority = compact(await read("engine/src/services/auth_service/durable_source/private_work/authority.rs"));
  const finish = body(authority, "pub(insuper::super)asyncfnfinish_from_source_for(");
  ordered(finish, ["self.source.verify(tx).await?", "self.relations.verify(tx).await?",
    "self.check_authority_for(authority_id,tx).await?", "self.recheck_session_for(authority_id,tx).await"]);
  const shared = compact(await read("engine/src/services/task_store/content_create.rs"));
  assert.ok(shared.includes("pub(crate)asyncfncreate_manual_content_in_transaction("));
  assert.ok(shared.includes("create_selected_in_transaction(tx,scope,None,task_id,owner,manifest,None).await"));
  ordered(shared, ["check_create_input(scope,task_id,owner,&manifest)?", "schema::check_schema_in_transaction(",
    "letnow=Utc::now()", "validate_content_record(scope,definitions,&snapshot)?", "records::encode(&snapshot)?",
    '"INSERTINTOcollaboration_tasks', "records::append_and_confirm("]);
  assert.doesNotMatch(shared, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|connect_lazy/);
});

test("empty Task Artifact metadata checks acquire and prove four locks without business SELECT or changing old readers", async () => {
  const schema = compact(await read("engine/src/services/artifact_store/schema.rs"));
  const plan = schema.match(/"PREPAREcyanrex_private_artifact_metadata_relations_v1AS([^";]+);DEALLOCATEcyanrex_private_artifact_metadata_relations_v1"/)?.[1];
  assert.ok(plan, "one bounded nonexecuting Simple Query batch");
  assert.deepEqual([...plan.matchAll(/ONLY(\w+?)(?=,|$)/g)].map(match => match[1]), [
    "collaboration_artifact_schema", "collaboration_artifacts", "collaboration_artifact_revisions", "collaboration_artifact_outbox",
  ]);
  assert.doesNotMatch(plan, /EXECUTE|\*|owner_id|head_revision_id|snapshot|manifest/);
  const metadata = body(schema, "pub(super)asyncfncheck_metadata_schema(");
  ordered(metadata, ["namespace(tx).await?", "Executor::execute(&mut**tx,METADATA_RELATION_LOCKS).await?",
    "count(DISTINCTl.relation)=4ASheld", "l.pid=pg_catalog.pg_backend_pid()", "l.database=",
    "n.nspname=current_schema()", "l.mode='AccessShareLock'ANDl.granted", "if!held",
    "check_selected_schema(scope,tx,lock).await"]);
  const full = body(schema, "pub(super)asyncfncheck_schema(scope:WorkspaceRef,");
  ordered(full, ["namespace(tx).await?", 'sqlx::query("LOCKTABLEcollaboration_artifact_schema,',
    "check_selected_schema(scope,tx,lock).await"]);
  const artifact = compact(await read("engine/src/services/artifact_store/mod.rs"));
  assert.ok(body(artifact, "pub(crate)asyncfnvalidate_artifact_namespace_in_transaction(")
    .includes("schema::check_metadata_schema(scope,tx,true).await"));
  const reader = compact(await read("engine/src/services/artifact_store/reader.rs"));
  assert.ok(body(reader, "pub(crate)asyncfnvalidate_namespace_in_transaction(")
    .includes("schema::check_schema(self.scope,tx,true).await"));
});

test("empty creation HTTP admits only strict POST title metadata and keeps same-origin response-lifetime admission", async () => {
  const source = compact(await read("engine/src/platform_http/task_create/mod.rs"));
  const handlers = compact(await read("engine/src/platform_http/task_create/handlers.rs"));
  assert.equal([...source.matchAll(/\.route\(/g)].length, 1);
  assert.ok(source.includes('route("/platform/v1/tasks",any(handlers::dispatch))'));
  assert.ok(source.includes("config::validate("));
  assert.ok(source.includes("Self::with_capacity(connector,4)"));
  assert.ok(source.includes("connector.create_session_empty_content_task(token,id,&title).await"));
  assert.ok(handlers.includes("#[serde(deny_unknown_fields)]structCreateEmpty{schema_version:CoreSchemaVersion,task_id:TaskId,title:String,}"));
  ordered(handlers, ['request.uri().path()!="/platform/v1/tasks"', "request.uri().query().is_some()", "Method::POST",
    "endpoint(&request)", "security::credentials(", "admission::acquire(&state.admission)",
    "handle(state,token,request).await", "admission::response(response,permit)",
    "security::request_bytes(request,false).await?", "contract::parse(&bytes)?",
    "TaskContentManifest::new(&request.title,vec![])", ".backend.create(&token,request.task_id,request.title)",
    "StatusCode::CREATED", "contract::SnapshotResponse::from(committed)"]);
  assert.doesNotMatch(source + handlers, /SET_COOKIE|Set-Cookie|timeout\(|build_task_content_router|create_session_artifact|revise_session_artifact|Method::(?:PUT|GET|PATCH|DELETE)/);
  const security = compact(await read("engine/src/platform_http/security.rs"));
  for (const value of ['"x-cyanrex-platform-request"', "method!=Method::GET", 'single(headers,"origin")',
    "headers.contains_key(header::AUTHORIZATION)", "Duration::from_secs(2)"])
    assert.ok(security.includes(value), value);
  const contract = compact(await read("engine/src/platform_http/contract.rs"));
  assert.ok(contract.includes("serde_json::from_slice::<MapOnly<T>>(bytes)"));
  const admission = compact(await read("engine/src/platform_http/admission.rs"));
  assert.ok(admission.includes(".try_acquire_owned()"));
  assert.ok(admission.includes("permit:Some(permit)"));
  assert.ok(admission.includes("self.inner.is_end_stream()"));
});

test("workspace creation stays separate from a sixth replacement capability and preserves every earlier split and capacity default", async () => {
  const pair = compact(await read("engine/src/platform_host/session_pair.rs"));
  for (const [name, count, last] of [
    ["split_platform_session_connectors", 1, "PlatformSessionConnector"],
    ["split_platform_workspace_connectors", 2, "PlatformTaskIndexConnector"],
    ["split_platform_content_connectors", 3, "PlatformTaskContentConnector"],
    ["split_platform_task_create_connectors", 4, "PlatformTaskCreateConnector"],
    ["split_platform_task_replace_connectors", 5, "PlatformTaskReplaceConnector"],
  ]) {
    const split = body(pair, `pub(crate)fn${name}(`);
    assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1, name);
    assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, count, name);
    assert.equal([...split.matchAll(/::from_shared\(/g)].length, count + 1, name);
    assert.ok(split.includes(`${last}::from_shared(storage)`), name);
    assert.doesNotMatch(split, /inspect_|verify_|\.connect\(|std::(?:env|fs)/);
  }
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.equal([...workspace.matchAll(/\.merge\(/g)].length, 6);
  assert.equal([...workspace.matchAll(/\.fallback\(/g)].length, 1);
  for (const value of ["Self::with_capacities(storage,4,4,4)", "Self::with_content_capacities(storage,login_capacity,lifecycle_capacity,index_capacity,4,)",
    "Self::with_create_capacities(storage,login_capacity,lifecycle_capacity,index_capacity,content_capacity,4,)",
    "Self::with_replace_capacities(storage,login_capacity,lifecycle_capacity,index_capacity,content_capacity,create_capacity,4,)",
    "TaskCreateHttpState::with_capacity(create,create_capacity)", "task_create::routes().with_state(state.create)"])
    assert.ok(workspace.includes(value), value);
  ordered(workspace, ["config::validate(storage.origin(),&[login_capacity,lifecycle_capacity,index_capacity,content_capacity,create_capacity,replace_capacity,]",
    "split_platform_task_replace_connectors(storage)", "SessionLoginHttpState::with_capacity("]);
  assert.ok(compact(await read("engine/src/platform_http/config.rs")).includes("!(1..=16).contains(capacity)"));
  const session = compact(await read("engine/src/platform_http/session_router/mod.rs"));
  assert.equal([...session.matchAll(/\.merge\(/g)].length, 2);
  assert.doesNotMatch(session, /TaskCreate|task_create|TaskReplace|task_content_replace|PlatformWorkspace/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /PlatformTaskCreateConnector|TaskCreateHttpState|build_task_create_router/, path);
});

test("all eight retained Task create SQL boundaries are named exactly in disposable CI", async () => {
  const directory = "engine/tests/platform_task_create_connection";
  const harness = await read("engine/tests/platform_task_create_connection_tdd.rs");
  const cases = [];
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_create_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_create_\w+)\(/g)];
    assert.equal(ignored.length, declared.length, file);
    for (const [, reason] of ignored)
      assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`#[path = "platform_task_create_connection/${file}"]`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
    assert.ok(source.split("\n").length <= 601, file);
  }
  assert.equal(cases.length, 8);
  const runner = await read("scripts/test-platform-task-create-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_create_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, selected.length);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/,
    /--locked --test platform_task_create_connection_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact/])
    assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-platform-task-create-connection\.sh/);
});

test("bilingual retained create guides keep empty creation distinct from payload saving and unknown outcome recovery", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-task-create-connection.md`);
    for (const value of ["issues/3", "PlatformTaskCreateConnector", "PlatformStorageConnector", "TaskCreateHttpState",
      "PlatformWorkspaceHttpState", "with_create_capacities", "READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE",
      "COMMIT", "READ ONLY", "/platform/v1/tasks", "1–16", "no-store", "no-cache", "nosniff", "SDK/OpenAPI",
      "unconfirmed", "CYANREX_TEST_DATABASE_URL", "platform-task-index-connection.md", "platform-task-content-connection.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /(?:code.*(?:optional|not required|not prerequisites)|(?:optional|not required).*code)/i : /代码.*(?:可选|不是|不要求)/);
    assert.match(guide, locale === "en" ? /(?:no|not).*browser/i : /浏览器.*(?:未|不)|不.*浏览器/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-task-create-connection.md`));
    for (const file of ["README.md", "architecture.md", "architecture-map.md", "platform-network.md", "testing-guide.md",
      "platform-loopback-host.md", "platform-loopback-tls-host.md"])
      assert.ok((await read(`docs/${locale}/${file}`)).includes("platform-task-create-connection.md"), `${locale}: ${file}`);
  }
});
