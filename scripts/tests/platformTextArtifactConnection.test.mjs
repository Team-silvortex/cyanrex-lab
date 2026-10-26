import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const ordered = (source, values) => {
  let offset = -1;
  for (const value of values) {
    offset = source.indexOf(value, offset + 1);
    assert.ok(offset >= 0, `present and ordered: ${value}`);
  }
};

test("retained text publication validates before connecting and confirms only after original guards", async () => {
  const source = code(await read("engine/src/platform_host/text_artifact_connection.rs"));
  const dense = compact(source);
  assert.match(source, /storage: Arc<PlatformStorageConnector>/);
  assert.match(source, /workers: Arc<Semaphore>/);
  assert.deepEqual([...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "scope", "create_session_text_artifact"]);
  assert.equal([...dense.matchAll(/\.storage\.connect\(\)/g)].length, 1);
  assert.ok(dense.includes("Semaphore::new(4)"));
  ordered(dense, ["valid_token(token)", "TextArtifactPublishWorkspace::draft(title,text)?", ".storage.connect()",
    "ArtifactContentWriter::from_reader(", 'begin_with("BEGINISOLATIONLEVELREADCOMMITTED")',
    "self.verify(&muttx).await?", "PreparedTextArtifactPublish::prepare(",
    "workspace.verify_root().await?", ".verify_artifact_root()", "self.verify(&muttx).await?",
    "prepared.finish(&muttx).await?", "tx.commit().await.map_err(DurableAuthError::from)?"]);
  const finish = dense.indexOf("prepared.finish(&muttx).await?");
  const commit = dense.indexOf("tx.commit().await.map_err(DurableAuthError::from)?", finish);
  assert.equal(dense.slice(finish, commit), "prepared.finish(&muttx).await?;");
  assert.doesNotMatch(source, /PgPool|PoolOptions|connect_lazy|\.validate_session\(|SET ROLE|SET TRANSACTION READ WRITE|std::(?:env|fs)|derive\([^)]*(?:Clone|Debug|Serialize)/);
});

test("new text uses current Session authority and validates exact current publication after deferred work", async () => {
  const source = compact(await read("engine/src/services/auth_service/durable_source/text_artifact_transaction.rs"));
  ordered(source, ["validate_text_bytes(text.as_bytes())", '"cyanrex.task-text"', '"text/plain"']);
  ordered(source, ["PrivateWorkContext::begin_index_for(", ".create_in_transaction(",
    'sqlx::query("SETCONSTRAINTSALLIMMEDIATE")', "workspace.writer.verify_in_transaction(tx,&pending).await?",
    "context.finish_retained_for(authority_id,tx).await?", "Ok(Self{"]);
  assert.ok(source.includes(".finish_from_source_for(self.authority_id,tx).await?"));
  assert.doesNotMatch(source, /PgPool|\.pool|\.connect\(|\.begin\(|\.commit\(|revise_session_artifact|TaskProvider|Runner|std::(?:env|fs)/);
});

test("retained text capability is explicit and does not change teaching or public contracts", async () => {
  assert.match(await read("engine/src/platform_host/mod.rs"), /pub use text_artifact_connection::PlatformTextArtifactConnector;/);
  for (const file of ["engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(code(await read(file)), /PlatformTextArtifactConnector|text_artifact_connection/, file);
  const workspace = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  assert.doesNotMatch(workspace, /PlatformTextArtifactConnector|text_artifact_connection/);
  assert.ok(workspace.includes("split_platform_text_publication_connectors(storage)"));
});

test("all retained text SQL boundaries are explicitly registered and selected once in CI", async () => {
  const directory = "engine/tests/platform_text_artifact_connection";
  const harness = await read("engine/tests/platform_text_artifact_connection_tdd.rs"), cases = [];
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_text_artifact_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_text_artifact_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason] of ignored) assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    if (ignored.length) assert.ok(harness.includes(`platform_text_artifact_connection/${file}`), file);
    cases.push(...ignored.map(([, , name]) => `${file.slice(0, -3)}::${name}`));
  }
  assert.equal(cases.length, 8);
  const runner = await read("scripts/test-platform-text-artifact-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_text_artifact_\w+)\b/gm)].map(match => match[1]);
  assert.equal(new Set(selected).size, 8);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_text_artifact_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const step = (await read(".github/workflows/ci.yml"))
    .split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /&& bash scripts\/test-platform-text-artifact-connection\.sh/);
});

test("retained text guides remain bilingual reachable and separate publication from Task saving", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const file = `docs/${locale}/platform-text-artifact-connection.md`, guide = await read(file);
    for (const value of ["issues/3", "PlatformTextArtifactConnector", "create_session_text_artifact", "cyanrex.task-text",
      "text/plain", "256 KiB", "READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE", "COMMIT", "READ ONLY",
      "CYANREX_TEST_DATABASE_URL", "platform-task-replace-connection.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-text-artifact-connection.md`));
    await assertDocumentationReference(`docs/${locale}/README.md`, file);
    await assertDocumentationReference(`docs/${locale}/project-status.md`, file);
  }
});
