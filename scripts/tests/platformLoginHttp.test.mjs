import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const modulePath = "engine/src/platform_http/session_login";

// Inventories supplement default transport and genuine restricted-role PostgreSQL behavior.
test("login HTTP consumes an explicit issuer without widening live or lifecycle composition", async () => {
  const source = await read(`${modulePath}/mod.rs`);
  assert.match(source, /pub fn new\(connector: PlatformLoginConnector\)/);
  assert.match(source, /let origin = connector\.origin\(\)\.to_owned\(\)/);
  assert.match(source, /Platform\(Arc<PlatformLoginConnector>\)/);
  assert.match(source, /#\[cfg\(test\)\]\s+Test\(Arc<test_support::Backend>\)/);
  assert.match(source, /rate: Arc<rate::LoginRate>/);
  assert.match(source, /admission: Arc<Semaphore>/);
  assert.match(source, /Self::with_capacity\(connector, 4\)/);
  const configured = source.slice(source.indexOf("fn configured("), source.indexOf("pub fn build_session_login_router("));
  assert.ok(configured.indexOf("config::validate(origin, &[max_in_flight])?;") < configured.indexOf("origin: origin.to_owned()"));
  assert.match(configured, /rate: Arc::new\(rate::LoginRate::new\(\)\)/);
  assert.match(configured, /admission: Arc::new\(Semaphore::new\(max_in_flight\)\)/);
  assert.equal([...configured.matchAll(/Semaphore::new\(/g)].length, 1);
  assert.doesNotMatch(configured, /Backend::Source|Backend::Platform|\.connect\(|\.await|\.or_else|\.unwrap/);
  assert.doesNotMatch(source, /PgPool|AuthService|DurableAuthSource|std::env|TcpListener|install_empty/);
  const index = await read("engine/src/platform_http/mod.rs");
  assert.match(index, /#\[cfg\(unix\)\]\s+mod session_login;/);
  assert.match(index, /#\[cfg\(unix\)\]\s+pub use session_login::\{build_session_login_router, SessionLoginHttpState\}/);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/bin/cyanrex-platform/main.rs", "engine/src/platform_http/session_lifecycle/mod.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(file), /SessionLoginHttpState|build_session_login_router|platform\/v1\/session\/login/, file);
});

test("login HTTP projects one bounded command directly to a sensitive Cookie", async () => {
  const source = await read(`${modulePath}/handlers.rs`);
  const dispatch = source.slice(source.indexOf("pub(super) async fn dispatch"), source.indexOf("async fn handle"));
  const order = ["endpoint(&request)", "headers::preflight(", "state.rate.admit()", "admission::acquire(", "handle(&state, request).await", "admission::response(response, permit)"];
  let last = -1;
  for (const value of order) { const index = dispatch.indexOf(value); assert.ok(index > last, value); last = index; }
  assert.match(source, /const MAX_REQUEST_BYTES: usize = 32 \* 1024/);
  assert.match(source, /#\[derive\(Deserialize\)\]\s+#\[serde\(deny_unknown_fields\)\]\s+struct LoginRequest/);
  assert.match(source, /input\.password\.len\(\) > 4096 \|\| input\.otp\.len\(\) > 64/);
  assert.equal([...source.matchAll(/\.login\(&input\.username, &input\.password, &input\.otp\)/g)].length, 1);
  assert.match(source, /HttpError::dispatched\(status, code\)/);
  assert.doesNotMatch(source, /tokio::time|validate_session|logout\(|expires_at|RetryAfter/);
  const headers = await read(`${modulePath}/headers.rs`);
  assert.match(headers, /Path=\/platform\/v1; HttpOnly; SameSite=Strict\{secure\}/);
  assert.match(headers, /cookie\.set_sensitive\(true\)/);
  assert.match(headers, /valid_token\(token\)/);
  assert.doesNotMatch(headers, /Domain=|Max-Age=|Expires=/);
  const rate = (await read(`${modulePath}/rate.rs`)).replace(/\/\/[^\n]*/g, "");
  assert.match(rate, /Duration::from_secs\(60\)/);
  assert.match(rate, /MAX_ATTEMPTS: u8 = 64/);
  assert.match(rate, /window: Mutex<Window>/);
  assert.doesNotMatch(rate, /HashMap|BTreeMap|forwarded|username|IpAddr|Semaphore::/i);
});

test("login HTTP SQL inventory is selected exactly by the disposable runner and CI", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_login_http", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_login_http/${file}`))
      .matchAll(/async fn (postgres_platform_login_http_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 7, "new SQL boundaries require deliberate runner coverage");
  const runner = await read("scripts/test-platform-login-http.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_login_http_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_login_http_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-login-http\.sh/);
});

test("bilingual login HTTP guides distinguish unmounted projection from deployed security", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-login-http.md`);
    for (const value of ["issues/3", "SessionLoginHttpState", "build_session_login_router", "64", "32 KiB",
      "4096", "409 session_present", "not_attempted", "unconfirmed", "COMMIT", "CYANREX_TEST_DATABASE_URL",
      "test-platform-login-http.sh", "platform-login-connection.md", "RFC 6265"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /router is unmounted/ : /路由仍未挂载/);
    assert.match(guide, locale === "en" ? /does not prove the browser/ : /不证明浏览器/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-login-http.md`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-login-http.md`));
  }
});
