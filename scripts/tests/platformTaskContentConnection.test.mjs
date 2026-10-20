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
    assert.ok(next >= 0 && next > offset, `present and ordered: ${value}`);
    offset = next;
  }
};

// These wiring inventories supplement real restricted-role SQL, file and HTTP regressions.
test("retained content connector derives private routing from one original storage without pool or writes", async () => {
  const source = code(await read("engine/src/platform_host/task_content_connection.rs"));
  assert.match(source, /pub struct PlatformTaskContentConnector\s*\{/);
  assert.ok(source.includes("storage: Arc<PlatformStorageConnector>"));
  assert.ok(source.includes("workers: Arc<Semaphore>"));
  assert.ok(compact(source).includes("workers:Arc::new(Semaphore::new(4))"));
  assert.deepEqual([...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "get_session_content_task"]);
  for (const value of ["Self::from_shared(Arc::new(storage))", "self.storage.config().origin()",
    "self.storage.config().scope()", "TaskContentReadWorkspace::new(",
    "self.storage.config().namespaces().task()", "self.storage.config().namespaces().artifacts()"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /PgPool|PoolOptions|connect_lazy|DurableAuthSource|AuthService|\.validate_session\(|inspect_|std::env|\.or_else\(|derive\([^)]*(?:Clone|Debug|Serialize)/);
  assert.doesNotMatch(source, /(?:create|replace|transition|publish|install|grant|execute)_session|SET TRANSACTION READ WRITE|SET ROLE/);
});

test("retained content runs byte and original-root checks before final fresh authority and confirmed commit", async () => {
  const source = compact(await read("engine/src/platform_host/task_content_connection.rs"));
  assert.equal([...source.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  ordered(source, ["valid_token(token)", ".storage.connect()",
    'begin_with("BEGINISOLATIONLEVELREADCOMMITTED")', "self.verify(&muttx).await?",
    "PreparedTaskContentRead::prepare(", "workspace.verify_root().await?",
    "self.verify(&muttx).await?", "prepared.finish(&muttx).await?",
    "tx.commit().await.map_err(DurableAuthError::from)?", "connection.close()"]);
  const finish = source.indexOf("prepared.finish(&muttx).await?");
  const commit = source.indexOf("tx.commit().await.map_err(DurableAuthError::from)?", finish);
  assert.equal(source.slice(finish, commit), "prepared.finish(&muttx).await?;");
  assert.ok(source.includes("tokio::time::timeout(Duration::from_secs(10)"));
  assert.ok(source.includes(".verify_transaction(tx).await"));
  assert.ok(source.includes(".verify_artifact_root()"));
  assert.doesNotMatch(source, /\.begin\(|\.validate_session\(|\.get_session_content_task\(token/);
});

test("pool-free reader copies only original descriptors and owns worker admission through actual blocking completion", async () => {
  const reader = compact(await read("engine/src/services/artifact_store/reader.rs"));
  for (const value of ["pub(crate)structArtifactContentReader", "from_retained_root(",
    "blob::BlobDirectory::from_retained_root(path,original)?", "Workers::Host(workers)",
    "Workers::Store=>None", ".try_acquire_owned()", "ArtifactStoreError::StorageUnavailable",
    "tokio::task::spawn_blocking(move||{let_permit=permit;operation(blobs)})",
    "schema::check_schema(self.scope,tx,lock).await?", "records::head(self.scope,tx,",
    "records::revision(", "self.read_blob(&revision).await?"])
    assert.ok(reader.includes(value), value);
  assert.doesNotMatch(reader.split("#[cfg(test)]")[0], /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|\.or_else\(|create_dir|read_dir|remove_file/);
  const blob = compact(await read("engine/src/services/artifact_store/blob.rs"));
  const retained = blob.split("pub(super)fnfrom_retained_root(")[1]?.split("pub(super)fnverify(")[0] ?? "";
  ordered(retained, ["original.try_clone()", "directory,", "retained.verify()?", "Ok(retained)"]);
  assert.doesNotMatch(retained, /open_root|Self::open|read_dir|read_to_end|write|create/);
  const host = compact(await read("engine/src/platform_host/storage_connection.rs"))
    + compact(await read("engine/src/platform_host/artifact_root.rs"));
  assert.ok(host.includes("ArtifactContentReader::from_retained_root("));
  assert.ok(host.includes("&self.file"), "the opaque original root lends only a retained reader");
  const units = await read("engine/src/services/artifact_store/reader_tests.rs");
  assert.equal([...units.matchAll(/#\[tokio::test\]/g)].length, 4);
  for (const value of ["task.abort()", "try_acquire_owned().is_err()", "release.send(())",
    "not-an-artifact", "from_retained_root(", "AtomicBool"])
    assert.ok(units.includes(value), value);
});

test("shared content reopening retains exact owned text and optional payload without requiring code or execution", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/task_content_transaction.rs"));
  ordered(source, ["PrivateWorkContext::begin_index_for(", "read_owned_content_task(",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(source.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  const shared = source.slice(source.indexOf("pub(super)asyncfnread_owned_content_task("));
  ordered(shared, ["get_content_in_transaction(", "context.select_target(tx,artifact_namespace).await?",
    "reader.validate_namespace_in_transaction(tx).await?", "ordered.sort_by_key(",
    ".read_in_transaction(tx,item.artifact(),owner)", "validate_text_payload_content(",
    "contents.sort_by_key(", "context.select_target(tx,task_namespace).await?",
    "get_content_in_transaction(", "Some(&snapshot)", "Ok(Some(SessionTaskContent{"]);
  assert.ok(shared.includes("returnOk(None)"), "a missing Task is not a content or namespace grant");
  assert.doesNotMatch(source, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|TaskProvider|Runner|TaskRun|ArtifactStore::open|std::(?:env|fs)/);
  const old = compact(await read("engine/src/services/auth_service/durable_source/task_commands/content.rs"));
  ordered(old.slice(old.indexOf("asyncfnexecute_content_task(")), [
    "ifmatches!(&operation,ContentOperation::Read)", "ArtifactContentReader::from_store(",
    "task_content_transaction::read_owned_content_task(", ".map(ContentPending::Read)", "self.pool.clone()",
  ]);
  const task = compact(await read("engine/src/services/task_store/content_read.rs"));
  ordered(task, ["check_scope(scope,reference,owner)?", "schema::check_schema_in_transaction(",
    "content::read_content_record(tx,scope,definitions,reference,owner,true).await"]);
  assert.doesNotMatch(task, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(/);
});

test("content HTTP has one read-only route and preserves same-origin private response-lifetime admission", async () => {
  const source = compact(await read("engine/src/platform_http/task_content_read/mod.rs"));
  const handlers = compact(await read("engine/src/platform_http/task_content_read/handlers.rs"));
  assert.ok(source.includes('"/platform/v1/tasks/{task_id}"'));
  assert.equal([...source.matchAll(/\.route\(/g)].length, 1);
  assert.ok(handlers.includes("Method::GET"));
  for (const value of ["security::credentials(", "admission::acquire(&state.admission)",
    "security::request_bytes(request,true).await?", ".backend.read(&token,id)",
    "SnapshotResponse::try_from(", "admission::response(response,permit)"])
    assert.ok(handlers.includes(value), value);
  assert.ok(source.includes("config::validate("));
  assert.ok(source.includes("connector.get_session_content_task(token,id).await"));
  assert.ok(source.includes("#[cfg(test)]Source{"));
  assert.ok(source.includes("#[cfg(test)]Self::Source{"));
  assert.deepEqual([...code(await read("engine/src/platform_http/task_content_read/mod.rs"))
    .matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
  ["new", "with_capacity", "build_task_content_read_router"]);
  assert.doesNotMatch(source.split("#[cfg(test)]")[0], /PgPool|DurableAuthSource|\.connect\(|\.or_else\(/);
  assert.doesNotMatch(source + handlers, /Method::POST|Method::PUT|Method::PATCH|Method::DELETE|create_session_content_task|replace_session_content_task|transition_session_content_task/);
});

test("workspace host preserves content reads beside empty creation and manifest replacement in six independent lanes", async () => {
  const source = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  for (const value of ["TaskContentReadHttpState", "task_content_read::routes()", "content_capacity",
    "TaskCreateHttpState", "task_create::routes()", "create_capacity",
    "TaskContentReplaceHttpState", "task_content_replace::routes()", "replace_capacity",
    "Self::with_capacities(storage,4,4,4)", "with_content_capacities(", "with_create_capacities(", "with_replace_capacities("])
    assert.ok(source.includes(value), value);
  assert.match(source, /\[login_capacity,lifecycle_capacity,index_capacity,content_capacity,create_capacity,replace_capacity,?\]/);
  assert.equal([...source.matchAll(/\.merge\(/g)].length, 6);
  assert.equal([...source.matchAll(/\.fallback\(/g)].length, 1);
  assert.doesNotMatch(source, /build_task_content_router|build_text_artifact_router|\.await|PgPool/);
  const pair = compact(await read("engine/src/platform_host/session_pair.rs"));
  const split = splitFunction(pair, "split_platform_content_connectors");
  assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, 3);
  assert.ok(split.includes("PlatformTaskContentConnector::from_shared(storage)"));
  assert.doesNotMatch(split, /TaskCreate|task_create|TaskReplace|task_replace/);
  assert.doesNotMatch(split, /inspect_|verify_|\.connect\(|std::(?:env|fs)/);
  const createSplit = splitFunction(pair, "split_platform_task_create_connectors");
  assert.equal([...createSplit.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...createSplit.matchAll(/Arc::clone\(&storage\)/g)].length, 4);
  assert.deepEqual([...createSplit.matchAll(/(Platform\w+Connector)::from_shared\(/g)].map(match => match[1]),
    ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector", "PlatformTaskCreateConnector"]);
  assert.ok(createSplit.includes("PlatformTaskCreateConnector::from_shared(storage)"));
  assert.doesNotMatch(createSplit, /TaskReplace|task_replace/);
  assert.doesNotMatch(createSplit, /inspect_|verify_|\.connect\(|std::(?:env|fs)/);
  const replaceSplit = splitFunction(pair, "split_platform_task_replace_connectors");
  assert.equal([...replaceSplit.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...replaceSplit.matchAll(/Arc::clone\(&storage\)/g)].length, 5);
  assert.deepEqual([...replaceSplit.matchAll(/(Platform\w+Connector)::from_shared\(/g)].map(match => match[1]),
    ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector", "PlatformTaskCreateConnector", "PlatformTaskReplaceConnector"]);
  assert.ok(replaceSplit.includes("PlatformTaskReplaceConnector::from_shared(storage)"));
  assert.doesNotMatch(replaceSplit, /inspect_|verify_|\.connect\(|std::(?:env|fs)/);
  const session = compact(await read("engine/src/platform_http/session_router/mod.rs"));
  assert.equal([...session.matchAll(/\.merge\(/g)].length, 2);
  assert.doesNotMatch(session, /TaskContentRead|task_content_read::routes|TaskCreate|task_create|TaskReplace|task_content_replace/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /PlatformTaskContentConnector|TaskContentReadHttpState|build_task_content_read_router/, path);
});

test("all eight retained Task content SQL boundaries are named exactly in disposable CI", async () => {
  const directory = "engine/tests/platform_task_content_connection";
  const harness = await read("engine/tests/platform_task_content_connection_tdd.rs");
  const cases = [];
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_content_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_content_\w+)\(/g)];
    assert.equal(ignored.length, declared.length, file);
    for (const [, reason] of ignored)
      assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`#[path = "platform_task_content_connection/${file}"]`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
    assert.ok(source.split("\n").length <= 601, file);
  }
  assert.equal(cases.length, 8, "new resource boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-task-content-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_content_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, selected.length);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/,
    /--locked --test platform_task_content_connection_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact/])
    assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-platform-task-content-connection\.sh/);
});

test("bilingual retained content guides distinguish reopening from saving sharing and execution", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-task-content-connection.md`);
    for (const value of ["issues/3", "PlatformTaskContentConnector", "PlatformStorageConnector",
      "TaskContentReadHttpState", "PlatformWorkspaceHttpState", "READ COMMITTED", "COMMIT",
      "/platform/v1/tasks/", "1–16", "no-store", "no-cache", "nosniff", "SDK/OpenAPI",
      "CYANREX_TEST_DATABASE_URL", "platform-task-index-connection.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /code.*(?:optional|not required)|(?:optional|not required).*code/i : /代码.*(?:可选|不是|不要求)/);
    assert.match(guide, locale === "en" ? /not.*(?:saving|save)|no.*(?:saving|save)/i : /不.*保存/);
    assert.match(guide, locale === "en" ? /worker.*(?:cancel|cancellation)|(?:cancel|cancellation).*worker/i : /(?:取消|worker|工作槽)/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-task-content-connection.md`));
    for (const file of ["README.md", "architecture.md", "architecture-map.md", "platform-network.md",
      "testing-guide.md", "platform-loopback-host.md", "platform-loopback-tls-host.md"])
      assert.ok((await read(`docs/${locale}/${file}`)).includes("platform-task-content-connection.md"), `${locale}: ${file}`);
  }
});
