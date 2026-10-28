import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

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

// Source inventories complement the explicitly selected real restricted-role SQL cases.
test("Task-bound text publication guards the exact owned Draft before bytes and retains final original authority", async () => {
  const connector = code(await read("engine/src/platform_host/task_text_artifact_connection.rs"));
  const dense = compact(connector);
  assert.deepEqual([...connector.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "create_session_task_text_artifact"]);
  assert.equal([...dense.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  assert.ok(dense.includes("Semaphore::new(4)"));
  ordered(dense, ["valid_token(token)", "TaskTextArtifactPublishWorkspace::draft(self.scope(),&expected,text)?",
    ".storage.connect()", 'begin_with("BEGINISOLATIONLEVELREADCOMMITTED")', "self.verify(&muttx).await?",
    "PreparedTaskTextArtifactPublish::prepare(", "workspace.verify_root().await?", ".verify_artifact_root()",
    "self.verify(&muttx).await?", "prepared.finish(&muttx).await?", "tx.commit().await.map_err(DurableAuthError::from)?"]);
  assert.ok(dense.includes("prepared.finish(&muttx).await?;tx.commit().await.map_err(DurableAuthError::from)?"));
  const transaction = compact(await read("engine/src/services/auth_service/durable_source/task_text_artifact_transaction.rs"));
  assert.equal([...transaction.matchAll(/PrivateWorkContext::begin_index_for\(/g)].length, 1);
  assert.equal([...transaction.matchAll(/get_content_in_transaction\(/g)].length, 2);
  assert.ok(transaction.includes("TextArtifactPublishWorkspace::draft(expected.task.title.clone(),text)"));
  ordered(transaction, ["PrivateWorkContext::begin_index_for(", "context.owner!=expected.task.owner",
    "get_content_in_transaction(", "old.task.revision!=expected.task.revision", "old.task.status!=TaskStatus::Draft",
    "old!=expected", "context.recheck_selected_for(authority_id,tx).await?",
    "context.select_target(tx,&workspace.artifacts).await?", ".create_in_transaction(",
    'sqlx::query("SETCONSTRAINTSALLIMMEDIATE")', "workspace.writer.verify_in_transaction(tx,&pending).await?",
    "context.select_target(tx,&workspace.task).await?", "get_content_in_transaction(", "Some(&old)",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(transaction.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  const authority = compact(await read("engine/src/services/auth_service/durable_source/private_work/authority.rs"));
  const selected = authority.slice(authority.indexOf("asyncfnrecheck_selected_for("), authority.indexOf("asyncfnfinish_from_source_for("));
  ordered(selected, ["selected.verify(tx).await?", "self.check_authority_for(authority_id,tx).await?",
    "self.recheck_session_for(authority_id,tx).await?", "NamespacePin::select(tx,&selected.name).await?", "selected.verify(tx).await?"]);
  assert.doesNotMatch(transaction, /\.pool|\.connect\(|\.begin\(|\.commit\(|replace_manual_content|replace_content_in_transaction|transition_in_transaction|insert_content_in_transaction/);
  assert.doesNotMatch(connector + transaction, /PgPool|PoolOptions|connect_lazy|\.validate_session\(|SET ROLE|SET TRANSACTION READ WRITE|std::(?:env|fs)|replace_manual_content|verify_pinned_contents|create_session_content_task|TaskProvider|Runner/);
});

test("Task-bound capability remains absent from ordinary UI while old five-field text POST and profile inventories stay compatible", async () => {
  assert.match(await read("engine/src/platform_host/mod.rs"), /pub use task_text_artifact_connection::PlatformTaskTextArtifactConnector;/);
  for (const path of ["engine/src/platform_http/text_artifact_publish/mod.rs", "engine/src/platform_http/text_artifact_publish/handlers.rs",
    "engine/src/bin/cyanrex-platform/main.rs", "engine/src/bin/cyanrex-platform/session_ui.rs",
    "engine/src/bin/cyanrex-platform/session_ui/task-edit.js", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(code(await read(path)), /PlatformTaskTextArtifactConnector|create_session_task_text_artifact|task_text_artifact_transaction/, path);
  const contract = await read("engine/src/platform_http/text_artifact/contract.rs");
  const body = contract.split("struct Create {")[1]?.split("\n}")[0] ?? "";
  assert.deepEqual([...body.matchAll(/^\s*(?:pub )?(\w+):/gm)].map(match => match[1]),
    ["schema_version", "artifact_id", "revision_id", "title", "text"]);
  assert.match(contract, /#\[serde\(deny_unknown_fields\)\]/);
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.equal([...workspace.matchAll(/\.merge\(/g)].length, 8);
  assert.ok(workspace.includes("with_text_publication_capacities("));
  const oldProfiles = workspace.slice(0, workspace.indexOf("pubfnwith_task_text_publication_capacities("));
  assert.equal([...oldProfiles.matchAll(/task_text:None/g)].length, 2);
  assert.doesNotMatch(oldProfiles, /task_text:Some\(/);
  const routes = [];
  for (const name of ["session_login", "session_lifecycle", "task_index", "task_content_read", "task_create", "task_content_replace", "text_artifact_publish"])
    routes.push(...[...compact(await read(`engine/src/platform_http/${name}/mod.rs`)).matchAll(/\.route\("([^"]+)"/g)].map(match => match[1]));
  assert.equal(routes.length, 8); assert.equal(new Set(routes).size, 8);
  const assets = code(await read("engine/src/bin/cyanrex-platform/session_ui.rs"));
  assert.equal([...assets.matchAll(/\.route\(/g)].length, 7);
  assert.equal([...assets.matchAll(/include_str!\(/g)].length, 7);
});

test("seven Task-bound real SQL boundaries are explicitly ignored inventoried and selected once by CI", async () => {
  const directory = "engine/tests/platform_task_text_artifact_connection";
  const harness = await read("engine/tests/platform_task_text_artifact_connection_tdd.rs"), cases = [];
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_text_artifact_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_text_artifact_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason] of ignored) assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`platform_task_text_artifact_connection/${file}`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
  }
  assert.equal(cases.length, 7);
  assert.equal([...harness.matchAll(/^#\[test\]/gm)].length, 1);
  const runner = await read("scripts/test-platform-task-text-artifact-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_text_artifact_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, 7); assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_task_text_artifact_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.equal([...step.matchAll(/bash scripts\/test-platform-task-text-artifact-connection\.sh/g)].length, 1);
  assert.ok(workflow.trimEnd().split("\n").length <= 600);
});

test("Task-bound publication guides remain bilingual mirrored and explicit about metadata-only guards", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const file = `docs/${locale}/platform-text-artifact-connection.md`, guide = await read(file);
    for (const value of ["PlatformTaskTextArtifactConnector", "create_session_task_text_artifact", "TaskContentSnapshot",
      "recheck_selected_for", "UPDATE(task_id)", "READ ONLY", "test-platform-task-text-artifact-connection.sh",
      locale === "en" ? "eight routes/seven lanes" : "八条路由／七个通道"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-text-artifact-connection.md`));
    const security = await read(`docs/${locale}/security.md`);
    assert.ok(security.includes("PlatformTaskTextArtifactConnector"));
    assert.ok(security.includes("UPDATE(task_id)"));
    assert.equal(security, await read(`frontend/public/course/${locale}/security.md`));
    await assertDocumentationReference(`docs/${locale}/security.md`, file);
  }
});
