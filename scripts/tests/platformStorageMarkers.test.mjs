import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = name => readFile(new URL(name, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function implementation() {
  let text = await read("engine/src/platform_host/storage_markers.rs");
  try {
    for (const name of await readdir(new URL("engine/src/platform_host/storage_markers", root))) {
      if (name.endsWith(".rs")) text += await read(`engine/src/platform_host/storage_markers/${name}`);
    }
  } catch (error) { if (error.code !== "ENOENT") throw error; }
  return codeOnly(text);
}

test("storage marker inspection reuses the original catalog in one owned read-only transaction", async () => {
  const code = await implementation();
  for (const name of ["catalog::require_idle", "catalog::collect", "config_matches", "namespace_oids",
    "REPEATABLE READ, READ ONLY", "is_in_transaction", "timeout", ".commit()", ".rollback()"])
    assert.ok(code.includes(name), name);
  const observation = code.match(/#\[derive\(([^)]*)\)\]\s*pub struct PlatformStorageMarkerObservation/);
  assert.ok(observation);
  assert.doesNotMatch(observation[1], /Debug|Clone|Serialize|Deserialize/);
  assert.doesNotMatch(code, /\.connect\(|PgConnectOptions|PgPool|install_empty|build_state|AuthService|ArtifactStore::open|std::env|std::fs|tokio::spawn|spawn_blocking/);
  assert.doesNotMatch(code, /set_config|SET ROLE|SET SESSION AUTHORIZATION|SET TRANSACTION READ WRITE|search_path\s*=/);
  const collect = code.slice(code.indexOf("async fn collect("), code.indexOf("async fn shape("));
  const lock = collect.indexOf("LOCK TABLE");
  const catalog = collect.indexOf("catalog::collect");
  assert.ok(lock >= 0 && catalog > lock, "candidate locks must precede the first catalog snapshot");
  assert.ok(collect.indexOf("shape(") > catalog && collect.indexOf("read_marker(") > catalog);
});

test("marker reads stay bounded and reject unsafe relations without fetching business or secret values", async () => {
  const code = await implementation();
  for (const name of ["collaboration_auth_source_schema", "collaboration_identity_schema",
    "collaboration_access_schema", "collaboration_task_schema", "collaboration_artifact_schema",
    "pg_catalog", "tableoid", "pg_typeof", "LIMIT 2", "ACCESS SHARE", "row_security",
    "to_regclass", "to_regnamespace", "relrowsecurity", "relforcerowsecurity", "pg_constraint",
    "pg_locks", "pg_backend_pid", "l.database", "l.granted", "AccessShareLock",
    "has_table_privilege", "TRIGGER"])
    assert.ok(code.includes(name), name);
  assert.doesNotMatch(code, /\b(?:CREATE|ALTER|DROP|GRANT|REVOKE|INSERT|UPDATE|DELETE)\s|FOR SHARE|FOR UPDATE|ACCESS EXCLUSIVE/i);
  assert.doesNotMatch(code, /FROM\s+(?:ONLY\s+)?(?:users|sessions|collaboration_tasks|collaboration_artifacts|collaboration_.*_outbox)\b|password_hash|totp_secret|otp_last_counter|token_digest|blob/i);
});

test("all storage marker SQL regressions are selected exactly in the existing disposable CI fixture", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_storage_markers", root))) {
    if (!file.endsWith(".rs")) continue;
    const text = await read(`engine/tests/platform_storage_markers/${file}`);
    for (const [, name] of text.matchAll(/async fn (postgres_platform_storage_markers_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 4);
  const runner = await read("scripts/test-platform-storage-markers.sh");
  const selected = [...runner.matchAll(/^  ((?:\w+::)*postgres_platform_storage_markers_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_storage_markers_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|pg_ctl|initdb|start\.sh/);
  const step = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /&& bash scripts\/test-platform-storage-markers\.sh/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
});

test("bilingual marker guides do not promote declared versions into full schema or authority health", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/platform-storage-markers.md`);
    for (const name of ["PlatformStorageMarkerObservation", "inspect_platform_storage_markers",
      "verify_platform_storage_markers", "READ ONLY", "issues/3"]) assert.ok(text.includes(name), name);
    const prose = text.replace(/\s+/g, " ");
    assert.match(prose, locale === "en" ? /not.*complete schema|not.*full schema/ : /不.*完整.*schema|不.*完整.*结构/);
    assert.match(text, locale === "en" ? /unmounted|no listener/ : /未挂载|不启动监听/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-storage-markers.md`);
  }
});
