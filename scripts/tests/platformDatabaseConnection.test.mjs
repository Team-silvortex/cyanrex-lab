import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("platform connections retain the original observation and publish only after single-attempt verification", async () => {
  const code = codeOnly(await read("engine/src/platform_host/database_connection.rs"));
  for (const value of ["original.config_matches(&config)", "original: PlatformCatalogObservation",
    "verify_platform_database(&self.config, &mut connection, original)", "PgConnection::connect_with(options)",
    "tokio::time::timeout(Duration::from_secs(10)", "Ok(connection)"])
    assert.ok(code.includes(value), value);
  assert.equal([...code.matchAll(/PgConnection::connect_with\(/g)].length, 1);
  assert.ok(code.indexOf("verify_platform_database(&self.config") < code.indexOf("Ok(connection)"));
  assert.doesNotMatch(code, /PgPool|PoolOptions|after_connect|connect_lazy|from_str|parse::<|std::env|set_var|remove_var|AuthService|build_state|TcpListener/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Debug|Clone|Serialize)[^\]]*\)\]\s*pub struct PlatformDatabaseConnector/);
  const exports = await read("engine/src/platform_host/mod.rs");
  for (const value of ["PlatformDatabaseConnector", "PlatformDatabaseEndpoint", "PlatformDatabaseConnectionError"])
    assert.ok(exports.includes(value), value);
});

test("local endpoint and explicit authentication builders reject inherited startup options without exporting secrets", async () => {
  const code = codeOnly(await read("engine/src/platform_host/database_connection.rs"));
  for (const value of ["password.len() > 4096", "password.contains('\\0')", "new_without_pgpass()",
    "get_options().is_some()", "AmbientStartupOptions", ".database(config.target().database())",
    ".username(config.target().role())", ".password(password)", "PgSslMode::Disable",
    '.application_name("cyanrex-platform-connection")', ".disable_statement_logging()",
    '("search_path", config.namespaces().source())', '("statement_timeout", "5000")',
    '("lock_timeout", "2000")', '("idle_in_transaction_session_timeout", "10000")'])
    assert.ok(code.includes(value), value);
  assert.ok(code.indexOf("get_options().is_some()") < code.indexOf(".apply(options)"));
  assert.doesNotMatch(code, /pub fn (?:options|password)|println!|eprintln!|tracing::|\.map_err\(\|(?:err|error)\|/);
  const endpoint = codeOnly(await read("engine/src/platform_host/database_connection/endpoint.rs"));
  for (const value of ["port == 0", "char::is_control", "part.is_empty()", 'matches!(part, "." | "..")',
    'format!("{text}/.s.PGSQL.{port}").len() > 100', "address.is_loopback()", ".socket(path)",
    ".host(&address.to_string())"])
    assert.ok(endpoint.includes(value), value);
  assert.doesNotMatch(endpoint, /lookup_host|ToSocketAddrs|std::env|std::fs/);
  assert.doesNotMatch(endpoint, /^\s*pub(?:\([^)]+\))?\s+(?:transport|port)\s*:/m);
});

test("all four real database selections run exactly in disposable PostgreSQL CI", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_database_connection", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`engine/tests/platform_database_connection/${file}`);
    for (const [, name] of source.matchAll(/async fn (postgres_platform_connection_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 4);
  const runner = await read("scripts/test-platform-database-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_connection_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_database_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-database-connection\.sh/);
  assert.ok(ci.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  const target = await read("engine/tests/platform_database_connection_tdd.rs");
  assert.match(target, /platform_host_preflight\/fixture\.rs/);
  assert.doesNotMatch(target, /platform_host_preflight_tdd\.rs/);
});

test("bilingual connection guides distinguish post-authentication identity checks from peer authentication and host acceptance", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/platform-database-connection.md`);
    for (const value of ["issues/3", "PGOPTIONS", "4096", "100", "Unavailable", "sqlx::postgres::notice",
      "platform-process-startup.md", "platform-host-preflight.md", "SCRAM", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(source.includes(value), value);
    const prose = source.replace(/\s+/g, " ");
    assert.match(prose, locale === "en" ? /after authentication, not before password delivery/ : /数据库认证之后核对，而不是在密码交付之前/);
    assert.match(prose, locale === "en" ? /not forced read-only/ : /不强制只读/);
    assert.match(prose, locale === "en" ? /not yet composed/ : /尚未与/);
    for (const path of ["README.md", "platform-host-preflight.md"])
      await assertDocumentationReference(`docs/${locale}/${path}`, `docs/${locale}/platform-database-connection.md`);
    assert.equal(source, await read(`frontend/public/course/${locale}/platform-database-connection.md`));
  }
});
