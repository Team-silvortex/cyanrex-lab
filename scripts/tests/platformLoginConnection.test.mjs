import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const auth = "engine/src/services/auth_service/durable_source";

// Inventories supplement actual restricted-role, transaction, wait and KDF-gap evidence.
test("bound login is a separate explicit capability without a pool or public issuer", async () => {
  const source = code(await read("engine/src/platform_host/login_connection.rs"));
  const methods = [...source.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]);
  assert.deepEqual(methods, ["new", "origin", "login"]);
  assert.ok(source.includes("storage: Arc<PlatformStorageConnector>"));
  assert.match(source, /pub\(super\) fn from_shared\(storage: Arc<PlatformStorageConnector>\)/);
  assert.match(source, /Self::from_shared\(Arc::new\(storage\)\)/);
  assert.ok(source.includes("context: LoginContext"));
  assert.ok(source.includes("LoginContext::new(storage.config().scope().authority_id)"));
  assert.doesNotMatch(source, /derive\([^)]*(Debug|Clone|Serialize|Deserialize)|PgPool|PoolOptions|connect_lazy|AuthService|std::env|set_var|TcpListener|install_empty|register\(/);
  const constructor = source.split("pub fn new(")[1]?.split("pub fn origin(")[0] ?? "";
  assert.doesNotMatch(constructor, /\.connect\(|inspect_|verify_|query\(|std::fs/);
  assert.ok((await read("engine/src/platform_host/mod.rs")).includes("PlatformLoginConnector"));
  const lifecycle = code(await read("engine/src/platform_host/session_connection.rs"));
  assert.deepEqual([...lifecycle.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]),
    ["new", "origin", "validate_session", "logout"]);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/bin/cyanrex-platform/main.rs", "engine/src/platform_http/session_lifecycle/mod.rs",
    "engine/src/platform_http/session_lifecycle/handlers.rs", "engine/openapi/openapi.json"])
    assert.doesNotMatch(code(await read(file)), /PlatformLoginConnector|platform\/v1\/session\/login/, file);
});

test("the original binding spans both physical connections and all final OTP proof precedes commit", async () => {
  const source = compact(await read("engine/src/platform_host/login_connection.rs"));
  const work = source.split("asyncfnlogin_work(")[1]?.split("asyncfnverify(")[0] ?? "";
  assert.equal([...work.matchAll(/\.storage\.connect\(\)/g)].length, 2);
  assert.equal([...work.matchAll(/begin_with\("BEGINISOLATIONLEVELREADCOMMITTED"\)/g)].length, 2);
  const prepare = work.indexOf("PreparedLoginRead::prepare(");
  const readCommit = work.indexOf("tx.commit().await?");
  const close = work.indexOf("connection.close().await");
  const password = work.indexOf(".verify_password(");
  assert.ok(prepare >= 0 && prepare < readCommit && readCommit < close && close < password);
  assert.equal([...work.matchAll(/self\.verify\(&muttx\)\.await\?/g)].length, 4);
  assert.ok(work.includes("Some(self.storage.config().limits())"));
  assert.ok(work.includes("self.verify(&muttx).await?;prepared.commit(&self.context,tx,otp).await"));
  assert.doesNotMatch(work, /SETTRANSACTIONREADWRITE|\.or_else\(|None\)/);
  const stages = compact(await read(`${auth}/login_transaction.rs`));
  const finish = stages.split("implPreparedLoginWrite{")[1] ?? "";
  const proof = finish.indexOf("self.pending_otp.recheck(");
  const commit = finish.indexOf("tx.commit().await?");
  assert.ok(proof >= 0 && proof < commit);
  assert.doesNotMatch(finish.slice(proof, commit), /\.await/);
  assert.ok(finish.indexOf("Ok(DurableLogin{") > commit);
  const pooled = compact(await read(`${auth}/sessions.rs`));
  for (const value of ["LoginContext::from_source(self)", "PreparedLoginRead::prepare(",
    ".verify_password(&context,password,otp)", "verified.prepare(&context,&muttx,otp,limits)",
    "prepared.commit(&context,tx,otp)"]) assert.ok(pooled.includes(value), value);
});

test("CI selects every retained login SQL and actual password-gap case exactly", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_login_connection", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`engine/tests/platform_login_connection/${file}`);
    for (const [, name] of source.matchAll(/async fn (postgres_platform_login_connection_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 12, "new SQL boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-login-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_login_connection_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  const gap = await read(`${auth}/platform_login_gap_sql_tests.rs`);
  const gapCases = [...gap.matchAll(/(?:async )?fn (postgres_platform_login_gap_\w+)\(/g)].map(match => match[1]);
  assert.equal(gapCases.length, 3);
  assert.deepEqual([...runner.matchAll(/^  (postgres_platform_login_gap_\w+)\b/gm)].map(match => match[1]).sort(), gapCases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_login_connection_tdd/,
    /--locked --lib/, /password_work::platform_login_gap_sql_tests/, /--ignored --list/, /grep -Fx/, /--ignored --exact/])
    assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-login-connection\.sh/);
});

test("bilingual login guides distinguish internal issuance from public delivery", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-login-connection.md`);
    for (const value of ["issues/3", "PlatformLoginConnector", "DurableLogin", "config.limits()",
      "COMMIT", "READ COMMITTED", "CYANREX_TEST_DATABASE_URL", "platform-session-connection.md"])
      assert.ok(guide.includes(value), value);
    assert.match(guide, locale === "en" ? /does not start a host/ : /不启动宿主/);
    assert.match(guide, locale === "en" ? /Public issuance still needs/ : /公共签发仍须/);
    assert.ok((await read(`docs/${locale}/README.md`)).includes("platform-login-connection.md"));
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-login-connection.md`));
  }
});
