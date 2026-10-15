import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const auth = "engine/src/services/auth_service/durable_source";
// These inventories supplement real SQL behavior; source spelling alone proves no quota.
const codeOnly = text => text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("Session stock is explicit trusted configuration without a legacy or HTTP switch", async () => {
  const module = await read(`${auth}/mod.rs`);
  const policy = codeOnly(await read(`${auth}/session_stock.rs`));
  assert.match(module, /pub use session_stock::SessionStockLimits;/);
  assert.match(policy, /pub struct SessionStockLimits/);
  assert.doesNotMatch(policy, /Deserialize|Default|std::env|\.connect\(|\.begin\(|\.commit\(|install_empty_schema|prune_expired_sessions/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/services/auth_service.rs", "engine/src/platform_http/session_lifecycle/handlers.rs",
    "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(await read(path), /SessionStockLimits|login_with_session_limits|\/platform\/v1\/session\/login/, path);
  }
});

test("bounded login retains one original protocol and checks stock around original writes", async () => {
  const sessions = codeOnly(await read(`${auth}/sessions.rs`));
  assert.match(sessions, /pub async fn login_with_session_limits\(/);
  assert.match(sessions, /login_inner\([\s\S]*?None\)/);
  assert.match(sessions, /login_inner\([\s\S]*?Some\(limits\)/);
  assert.match(sessions, /SET TRANSACTION ISOLATION LEVEL READ COMMITTED/);
  assert.match(sessions, /SET CONSTRAINTS ALL IMMEDIATE/);
  assert.match(sessions, /SessionSourcePin::capture/);
  assert.match(sessions, /consume_login_otp/);
  assert.match(sessions, /recheck_otp_consumption/);
  const prepare = sessions.indexOf("self.prepare_otp_consumption");
  const before = sessions.indexOf("limits.check_before", prepare);
  const consume = sessions.indexOf("consume_login_otp", before);
  const insertion = sessions.indexOf("INSERT INTO sessions", consume);
  const flush = sessions.indexOf("SET CONSTRAINTS ALL IMMEDIATE", insertion);
  const sourceReadback = sessions.indexOf("pin.verify", flush);
  const selectedSession = sessions.indexOf("self.session_record", sourceReadback);
  const after = sessions.indexOf("stock.verify_after", selectedSession);
  const proof = sessions.indexOf("self.recheck_otp_consumption", after);
  const commit = sessions.indexOf("tx.commit().await?", proof);
  assert.ok(prepare >= 0 && prepare < before && before < consume && consume < insertion &&
    insertion < flush && flush < sourceReadback && sourceReadback < selectedSession &&
    selectedSession < after && after < proof && proof < commit);
  assert.doesNotMatch(sessions.slice(proof, commit), /\.await/);
  for (const statement of ["SET TRANSACTION ISOLATION LEVEL READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE"]) {
    assert.match(sessions, new RegExp(`if limits\\.is_some\\(\\)\\s*\\{[^{}]*${statement}`));
  }
  assert.equal([...sessions.matchAll(/\bverify_password\(/g)].length, 1);
  assert.doesNotMatch(sessions, /prune_expired_sessions|DELETE FROM sessions[\s\S]*?expires_at|SET TRANSACTION READ WRITE/);
  const stock = codeOnly(await read(`${auth}/session_stock.rs`));
  assert.match(stock, /COUNT\(\*\)|count\(\*\)/);
  assert.match(stock, /LIMIT \$\d/);
  assert.match(stock, /SELECT 1 FROM sessions LIMIT \$1/);
  assert.match(stock, /SELECT 1 FROM sessions WHERE account_id = \$1 LIMIT \$2/);
  assert.doesNotMatch(stock, /JOIN users|expires_at|clock_timestamp|password_hash|totp_secret|SELECT token/);
});

test("every stock SQL regression is selected exactly by the disposable runner and CI", async () => {
  const directory = `${auth}/session_stock_sql`;
  const cases = [];
  for (const entry of await readdir(new URL(directory, root))) {
    if (!entry.endsWith(".rs")) continue;
    const contents = await read(`${directory}/${entry}`);
    const prefix = entry === "mod.rs" ? "" : `${entry.slice(0, -3)}::`;
    for (const [, name] of contents.matchAll(/async fn (postgres_session_stock_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
      assert.match(contents, /#\[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"\]/);
    }
  }
  assert.equal(cases.length, 8, "stock, post-write and actual concurrency need deliberate coverage");
  const runner = await read("scripts/test-durable-session-stock.sh");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_stock_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--locked --lib/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact/);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-session-stock\.sh/);
  assert.match(step, /&& bash scripts\/test-platform-session-http\.sh/);
});

test("stock documentation separates stored limits from deployment and automatic cleanup", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const doc = await read(`docs/${locale}/durable-session-stock.md`);
    for (const value of ["SessionStockLimits", "login_with_session_limits", "RateLimited",
      "StorageUnavailable", "READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE", "issues/3"]) assert.ok(doc.includes(value), value);
    assert.match(doc, locale === "en" ? /expired|expiry/ : /过期/);
    assert.match(doc, locale === "en" ? /unmounted|no host|not a host/ : /未挂载|不启动宿主/);
    assert.ok((await read(`docs/${locale}/README.md`)).includes("durable-session-stock.md"));
  }
});
