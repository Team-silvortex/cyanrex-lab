import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("host preflight is a separate explicit library boundary with no runtime composition", async () => {
  const library = await read("engine/src/lib.rs");
  assert.match(library, /#\[cfg\(unix\)\]\s*pub mod platform_host;/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "frontend/pages/settings.tsx"]) {
    assert.doesNotMatch(await read(path), /PlatformHostPreflightConfig|inspect_platform_database|verify_platform_database/, path);
  }
  const config = codeOnly(await read("engine/src/platform_host/config.rs"));
  assert.match(config, /platform_origin::validate\(\s*origin\s*\)/);
  assert.doesNotMatch(config, /platform_http|validate_platform_origin/);
  assert.match(config, /valid_platform_namespace/);
  assert.match(config, /SessionStockLimits/);
  assert.doesNotMatch(config, /Deserialize|Serialize|Default|std::env|\.connect\(|std::fs|PgConnectOptions|database_password/);
});

test("catalog observation uses read-only owned transactions without business reads or privilege mutation", async () => {
  const code = codeOnly(await read("engine/src/platform_host/catalog.rs"));
  assert.match(code, /REPEATABLE READ, READ ONLY/);
  assert.match(code, /pg_catalog\.pg_control_system\(\)/);
  assert.match(code, /pg_catalog\.pg_namespace/);
  assert.match(code, /pg_catalog\.pg_roles/);
  assert.match(code, /pg_catalog\.pg_auth_members/);
  assert.match(code, /pg_catalog\.has_database_privilege/);
  assert.match(code, /pg_catalog\.has_schema_privilege/);
  for (const field of ["rolsuper", "rolbypassrls", "rolcreaterole", "rolcreatedb", "rolreplication",
    "rolcanlogin", "session_user", "current_user", "database_oid", "datdba", "nspowner",
    "CONNECT", "CREATE", "TEMP", "pg_my_temp_schema", "current_schemas",
    "statement_timeout", "lock_timeout", "is_in_transaction"]) assert.ok(code.includes(field), field);
  assert.match(code, /tokio::time::timeout|timeout\(/);
  assert.match(code, /SAVEPOINT cyanrex_platform_preflight_probe; RELEASE SAVEPOINT cyanrex_platform_preflight_probe/);
  assert.match(code, /25P01/);
  assert.match(code, /25P02/);
  assert.match(code, /Executor::execute/);
  assert.doesNotMatch(code, /\.connect\(|PgConnectOptions|\.install|AuthService|build_state|TcpListener|spawn_blocking|tokio::spawn|std::env|std::fs/);
  assert.doesNotMatch(code, /\b(?:CREATE|ALTER|DROP|GRANT|REVOKE|INSERT|UPDATE|DELETE)\s|pg_authid|rolpassword|FROM\s+(?:users|sessions|tasks|artifact_)/i);
  assert.doesNotMatch(code, /set_config|SET ROLE|SET SESSION AUTHORIZATION|SET TRANSACTION READ WRITE/);
  const observation = code.match(/#\[derive\(([^)]*)\)\]\s*pub struct PlatformCatalogObservation/);
  assert.ok(observation);
  assert.doesNotMatch(observation[1], /Debug|Clone|Serialize|Deserialize/);
});

test("all catalog SQL cases have exact disposable selectors and existing CI coverage", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_host_preflight", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`engine/tests/platform_host_preflight/${file}`);
    for (const [, name] of source.matchAll(/async fn (postgres_host_preflight_\w+)\(/g)) cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 4, "catalog-only scope needs deliberate role, target and replacement coverage");
  const runner = await read("scripts/test-platform-host-preflight.sh");
  const selected = [...runner.matchAll(/^  ((?:\w+::)*postgres_host_preflight_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test platform_host_preflight_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact/);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /&& bash scripts\/test-platform-host-preflight\.sh/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
});

test("bilingual preflight documentation distinguishes catalog identity from healthy storage", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const document = await read(`docs/${locale}/platform-host-preflight.md`);
    for (const name of ["PlatformHostPreflightConfig", "inspect_platform_database", "verify_platform_database",
      "pg_control_system", "READ ONLY", "issues/3"]) assert.ok(document.includes(name), name);
    assert.match(document, locale === "en" ? /catalog-only/ : /只读目录|仅目录/);
    assert.match(document, locale === "en" ? /unmounted|no listener/ : /未挂载|不启动监听/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-host-preflight.md`);
  }
});
