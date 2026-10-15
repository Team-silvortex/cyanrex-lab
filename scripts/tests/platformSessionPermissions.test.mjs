import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = text => text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("Source-only relation locking uses a fixed nonexecuting Simple Query batch", async () => {
  const source = codeOnly(await read("engine/src/services/auth_service/durable_source/source_relations.rs"));
  assert.match(source, /Executor::execute\(\s*&mut \*\*tx,/);
  assert.match(source, /PREPARE cyanrex_session_source_relations_v1 AS\s+SELECT 1 FROM ONLY users, ONLY sessions,\s+ONLY collaboration_auth_source_schema LIMIT 0;\s+DEALLOCATE cyanrex_session_source_relations_v1/);
  assert.equal([...source.matchAll(/PREPARE cyanrex_session_source_relations_v1/g)].length, 1);
  assert.doesNotMatch(source, /EXECUTE cyanrex|EXPLAIN|DEALLOCATE ALL|GRANT |REVOKE |\.prepare\(|format!|Uuid::new/);
});

test("nonexecuting capture preserves original shape identity and actual transaction-lock checks", async () => {
  const source = codeOnly(await read("engine/src/services/auth_service/durable_source/source_relations.rs"));
  for (const value of ["if registry", "LOCK TABLE users, sessions, collaboration_auth_source_schema",
    "collaboration_identity_audit, collaboration_policy_audit IN ACCESS SHARE MODE", "to_regclass(e.relation)::oid = t.oid",
    "relpersistence", "relrowsecurity", "relforcerowsecurity", "relhassubclass", "relispartition", "pg_inherits",
    "pg_constraint", "pg_locks", "pg_backend_pid", "pg_database", "pg_catalog.current_database()",
    "l.relation::bigint = ANY($1)", "l.mode='AccessShareLock'", "l.granted", "Self::capture(tx, self.registry)"])
    assert.ok(source.includes(value), value);
  const metadata = source.split("let rows = sqlx::query(")[1]?.split("let mut oids")[0] ?? "";
  assert.ok(metadata.includes("pg_constraint"));
  assert.doesNotMatch(metadata, /pg_locks/);
  assert.match(source, /oids\.push\(row\.try_get\("oid"\)\?\);\s*}\s*if !registry\s*\{/);
  assert.match(source, /oids\.len\(\) != 3/);
  assert.match(source, /SELECT count\(DISTINCT l\.relation\)=3 AS held/);
  assert.match(source, /\.bind\(&oids\)/);
  assert.doesNotMatch(source, /SET ROLE|search_path\s*=|install_empty|\.login\(|\.register\(|DEALLOCATE ALL/);
});

test("all restricted Session SQL selections run exactly in the existing disposable CI fixture", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_session_permissions", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`engine/tests/platform_session_permissions/${file}`);
    for (const [, name] of source.matchAll(/async fn (postgres_session_permissions_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 4);
  const runner = await read("scripts/test-platform-session-permissions.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_session_permissions_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_session_permissions_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-session-permissions\.sh/);
  assert.ok(ci.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
});

test("bilingual role guides distinguish credential restrictions from read-only roles and mounted hosts", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/platform-session-permissions.md`);
    for (const value of ["cyanrex_session_source_relations_v1", "password_salt", "password_hash", "totp_secret",
      "otp_last_counter", "42501", "UPDATE", "PREPARE", "REPEATABLE READ", "issues/3"])
      assert.ok(source.includes(value), value);
    const prose = source.replace(/\s+/g, " ");
    assert.match(prose, locale === "en" ? /not a read-only role/ : /不是只读角色/);
    assert.match(prose, locale === "en" ? /does not start a host/ : /不启动宿主/);
    assert.ok((await read(`docs/${locale}/README.md`)).includes("platform-session-permissions.md"));
  }
});
