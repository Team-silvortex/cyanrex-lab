import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");

// Wiring inventories supplement real response lifetimes, SQL authorization and cancellation.
test("combined Session state accepts one binding and validates both lanes before internal sharing", async () => {
  const source = code(await read("engine/src/platform_http/session_router/mod.rs"));
  const methods = [...source.matchAll(/pub fn (\w+)\(/g)].map(match => match[1]);
  assert.deepEqual(methods, ["new", "with_capacities", "build_platform_session_router"]);
  assert.match(source, /Self::with_capacities\(storage, 4, 4\)/);
  assert.match(source, /#\[derive\(Clone\)\]\s+pub struct PlatformSessionHttpState/);
  assert.match(source, /login: SessionLoginHttpState,\s+lifecycle: SessionLifecycleHttpState/);
  const sequence = ["config::validate(storage.origin(), &[login_capacity, lifecycle_capacity])?",
    "split_platform_session_connectors(storage)", "SessionLoginHttpState::with_capacity(login, login_capacity)",
    "SessionLifecycleHttpState::with_platform_connector("];
  let previous = -1;
  for (const value of sequence) {
    const current = source.indexOf(value, previous + 1);
    assert.ok(current > previous && current >= 0, value);
    previous = current;
  }
  assert.doesNotMatch(source, /DurableAuthSource|PgPool|AuthService|Engine|TcpListener|std::env|install_|\.connect\(|\.await\?|tokio::spawn|Semaphore::new/);
});

test("shared storage remains private and composition merges only fallback-free routes", async () => {
  const pair = compact(await read("engine/src/platform_host/session_pair.rs"))
    .split("pub(crate)fnsplit_platform_workspace_connectors(")[0];
  assert.ok(pair.includes("pub(crate)fnsplit_platform_session_connectors("));
  assert.equal([...pair.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...pair.matchAll(/Arc::clone\(&storage\)/g)].length, 1);
  for (const value of ["PlatformLoginConnector::from_shared(Arc::clone(&storage))",
    "PlatformSessionConnector::from_shared(storage)"]) assert.ok(pair.includes(value));
  assert.doesNotMatch(pair, /inspect_|verify_|\.connect\(|std::env|std::fs|\.clone\(|pubfn/);
  const storage = code(await read("engine/src/platform_host/storage_connection.rs"));
  assert.match(storage, /pub\(crate\) fn origin\(&self\) -> &str/);
  assert.doesNotMatch(storage, /pub fn origin|derive\([^)]*(?:Debug|Clone|Serialize)\)\]\s+pub struct PlatformStorageConnector/);
  const router = compact(await read("engine/src/platform_http/session_router/mod.rs"));
  for (const value of [".merge(session_login::routes().with_state(state.login))",
    ".merge(session_lifecycle::routes().with_state(state.lifecycle))", ".fallback(not_found)",
    ".layer(middleware::from_fn(security::response_headers))"]) assert.ok(router.includes(value), value);
  assert.equal([...router.matchAll(/\.fallback\(/g)].length, 1);
  assert.equal([...router.matchAll(/\.layer\(/g)].length, 1);
  assert.equal([...router.matchAll(/\.merge\(/g)].length, 2);
  assert.doesNotMatch(router, /TaskIndex|TaskCreate|TaskReplace|TaskContent(?:Index|Read|Replace)?HttpState|task_index|task_content|task_create|catalog_task|text_artifact|file_distribution|\/platform\/v1\/tasks/,
    "Session-only composition must not grow Task, Artifact, file or resource routes");
  for (const [module, name] of [["session_login", "SessionLoginHttpState"], ["session_lifecycle", "SessionLifecycleHttpState"]]) {
    const source = code(await read(`engine/src/platform_http/${module}/mod.rs`));
    const routes = source.split(`pub(super) fn routes() -> Router<${name}>`)[1] ?? "";
    assert.match(routes, /Router::new\(\)/);
    assert.doesNotMatch(routes, /\.fallback\(|\.layer\(|\.with_state\(/);
    assert.match(source.split("pub(super) fn routes()")[0], /routes\(\)[\s\S]*\.fallback\(handlers::dispatch\)[\s\S]*\.layer\(middleware::from_fn\(security::response_headers\)\)[\s\S]*\.with_state\(state\)/);
  }
});

test("combined Session router remains unmounted and every SQL case is selected exactly", async () => {
  const index = await read("engine/src/platform_http/mod.rs");
  assert.match(index, /#\[cfg\(unix\)\]\s+mod session_router;/);
  assert.match(index, /#\[cfg\(unix\)\]\s+pub use session_router::\{build_platform_session_router, PlatformSessionHttpState\}/);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/bin/cyanrex-platform/main.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(file), /PlatformSessionHttpState|build_platform_session_router|split_platform_session_connectors/, file);
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_session_router", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_session_router/${file}`))
      .matchAll(/async fn (postgres_platform_session_router_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 7, "new SQL boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-session-router.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_session_router_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_session_router_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-session-router\.sh/);
});

test("shared Session documentation distinguishes independent lanes and unknown browser delivery", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-session-router.md`);
    for (const value of ["issues/3", "PlatformSessionHttpState", "build_platform_session_router", "with_capacities",
      "64", "fallback", "COMMIT", "CYANREX_TEST_DATABASE_URL", "platform-storage-open.md", "platform-login-http.md"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /remains\s+unmounted/ : /路由仍未挂载/);
    assert.match(guide, locale === "en" ? /does not prove Cookie delivery/ : /不证明浏览器收到了/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-session-router.md`));
    for (const path of ["README.md", "platform-login-http.md", "platform-storage-open.md", "architecture-map.md", "testing-guide.md"])
      await assertDocumentationReference(`docs/${locale}/${path}`, `docs/${locale}/platform-session-router.md`);
  }
});
