import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => source.replace(/\s+/g, "");

// Structural inventories supplement, never replace, real SQL/HTTP transaction evidence.
test("platform Session composition owns one retained storage binding without issuing or pooling", async () => {
  const code = codeOnly(await read("engine/src/platform_host/session_connection.rs"));
  for (const value of ["pub struct PlatformSessionConnector", "storage: PlatformStorageConnector",
    "pub fn new(storage: PlatformStorageConnector)", "self.storage.config().origin()",
    "self.storage.config().scope().authority_id", "DurableAuthError::StorageUnavailable"])
    assert.ok(code.includes(value), value);
  const methods = [...code.matchAll(/pub (?:async )?fn (\w+)\(/g)].map(match => match[1]);
  assert.deepEqual(methods, ["new", "origin", "validate_session", "logout"]);
  const construction = code.split("pub fn new(")[1]?.split("pub fn origin(")[0] ?? "";
  assert.doesNotMatch(construction, /\.connect\(|inspect_|verify_|std::fs|std::env|query\(/);
  assert.doesNotMatch(code, /derive\([^)]*(?:Debug|Clone|Serialize|Deserialize)[^)]*\)\]\s*pub struct PlatformSessionConnector/);
  assert.doesNotMatch(code, /PgPool|PoolOptions|connect_lazy|after_connect|DurableAuthSource|AuthService|std::env|set_var|remove_var|TcpListener|build_state|\.or_else\(|\.(?:login|register|install_empty_schema|prune_expired_sessions)\s*\(/);
  assert.ok((await read("engine/src/platform_host/mod.rs")).includes("PlatformSessionConnector"));
});

test("Session preparation and final freshness stay inside one transaction between retained guards and commit", async () => {
  const code = codeOnly(await read("engine/src/platform_host/session_connection.rs"));
  for (const [name, prepared, next] of [
    ["validate_session", "PreparedSessionRead", "pub async fn logout("],
    ["logout", "PreparedSessionLogout", "async fn verify("],
  ]) {
    const body = compact(code.split(`pub async fn ${name}(`)[1]?.split(next)[0] ?? "");
    assert.equal([...body.matchAll(/\.storage\.connect\(\)/g)].length, 1, name);
    assert.ok(body.includes('begin_with("BEGINISOLATIONLEVELREADCOMMITTED")'), name);
    const begin = body.indexOf(".begin_with(");
    const guards = [...body.matchAll(/self\.verify\(&muttx\)\.await\?/g)].map(match => match.index);
    const prepare = body.indexOf(`${prepared}::prepare(`);
    const finish = body.indexOf("prepared.finish(&muttx).await?");
    const commit = body.indexOf("tx.commit().await?");
    assert.equal(guards.length, 2, name);
    assert.ok(begin >= 0 && begin < guards[0] && guards[0] < prepare && prepare < guards[1]
      && guards[1] < finish && finish < commit, name);
    assert.equal(body.slice(finish, commit), "prepared.finish(&muttx).await?;", name);
    assert.ok(body.indexOf("close(connection).await") > commit, name);
    assert.ok(body.includes("tokio::time::timeout(Duration::from_secs(10)"), name);
    if (name === "logout") {
      const deferred = body.indexOf('query("SETCONSTRAINTSALLIMMEDIATE")');
      assert.ok(prepare < deferred && deferred < guards[1]);
    }
  }
  const storage = compact(codeOnly(await read("engine/src/platform_host/storage_connection.rs")));
  assert.ok(storage.includes("verify_in_transaction(&self.config,tx,&self.original_markers)"));
  assert.ok(compact(code).includes("self.storage.verify_transaction(tx).await"));
  const markers = codeOnly(await read("engine/src/platform_host/storage_markers.rs"));
  const retained = markers.split("pub(super) async fn verify_in_transaction(")[1]?.split("async fn observe_owned(")[0] ?? "";
  assert.ok(compact(retained).includes("expected.matches(&collect(config,tx,CatalogBinding::Observed(&expected.catalog),Some(&expected.markers),).await?,)"));
  assert.doesNotMatch(retained, /\.begin|\.commit|inspect_platform_storage_markers\(/);
  const prepared = codeOnly(await read("engine/src/services/auth_service/durable_source/session_transaction.rs"));
  for (const value of ["SessionSourcePin::capture_for(authority_id, tx, false)",
    "SessionSourcePin::capture_for(authority_id, tx, true)", "self.pin.verify_for(self.authority_id, tx, false)",
    "self.pin.verify_for(self.authority_id, tx, true)", "record(self.authority_id, tx, &self.digest).await",
    "expires_at <= DurableAuthSource::now(connection).await?", "FOR SHARE OF s, u"])
    assert.ok(prepared.includes(value), value);
  assert.doesNotMatch(prepared, /\.commit\(|PgPool|\.connect\(|pub async fn (?:prepare|finish)/);
});

test("the explicit HTTP backend shares only its selected connector and keeps failed logout cookies intact", async () => {
  const state = codeOnly(await read("engine/src/platform_http/session_lifecycle/mod.rs"));
  for (const value of ["enum Backend", "Source(DurableAuthSource)", "Platform(Arc<PlatformSessionConnector>)",
    "pub fn with_platform_storage(", "PlatformSessionConnector::new(storage)", "connector.origin().to_owned()",
    "Self::configured(Backend::Platform(connector), &origin, max_in_flight)",
    "Self::Source(source) => source.validate_session(token).await",
    "Self::Platform(connector) => connector.validate_session(token).await",
    "Self::Source(source) => source.logout(token).await", "Self::Platform(connector) => connector.logout(token).await"])
    assert.ok(state.includes(value), value);
  assert.doesNotMatch(state, /\.or_else\(|PgPool|connect_lazy|std::env|\.connect\(/);
  const handler = compact(codeOnly(await read("engine/src/platform_http/session_lifecycle/handlers.rs")));
  assert.ok(handler.includes("state.backend.validate_session(&token).await.map_err(source_error)?"));
  const logout = handler.split("Endpoint::Logout=>")[1] ?? "";
  const dispatched = logout.indexOf("state.backend.logout(&token).await.map_err(source_error)?");
  assert.ok(dispatched >= 0 && dispatched < logout.indexOf("header::SET_COOKIE"));
  assert.ok(handler.includes('HttpError::dispatched(StatusCode::SERVICE_UNAVAILABLE,"service_unavailable")'));
  for (const file of ["engine/src/application.rs", "engine/src/state.rs", "engine/src/main.rs",
    "engine/src/bin/cyanrex-platform/main.rs", "engine/openapi/openapi.json"])
    assert.doesNotMatch(codeOnly(await read(file)), /PlatformSessionConnector|with_platform_storage|build_session_lifecycle_router/, file);
});

test("seven exact disposable Session SQL selections and default compatibility tests remain in existing CI", async () => {
  const defaults = await read("engine/tests/platform_session_backend_tdd.rs");
  assert.equal([...defaults.matchAll(/(?:async )?fn (platform_session_http_\w+)\(/g)].length, 5);
  const target = await read("engine/tests/platform_session_connection_tdd.rs");
  assert.ok(target.includes("platform_session_connection/"));
  assert.doesNotMatch(target, /platform_session_http_tdd\.rs|platform_storage_connection_tdd\.rs/);
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_session_connection", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = codeOnly(await read(`engine/tests/platform_session_connection/${file}`));
    for (const [, name] of source.matchAll(/^\s*async fn (postgres_platform_session_connection_\w+)\(/gm))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 7);
  const runner = await read("scripts/test-platform-session-connection.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_session_connection_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_session_connection_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-session-connection\.sh/);
  const runners = [...ci.matchAll(/bash (scripts\/[\w-]+\.sh)/g)].map(match => match[1]);
  assert.equal(new Set(runners).size, runners.length);
});

test("bilingual Session connection guides and generated copies retain the current authorization boundary", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-session-connection.md`);
    for (const value of ["issues/3", "PlatformSessionConnector", "with_platform_storage", "READ COMMITTED",
      "COMMIT", "unconfirmed", "platform-storage-connection.md", "platform-session-lifecycle.md", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), value);
    const prose = guide.replace(/\s+/g, " ");
    assert.match(prose, locale === "en" ? /unmounted|not mounted|no listener or issuer is started/i : /未挂载|不启动监听或签发方/);
    assert.match(prose, locale === "en" ? /not public partial commands or reusable authorization/i : /不公开半完成命令或可复用权限/);
    assert.ok((await read(`docs/${locale}/README.md`)).includes("platform-session-connection.md"));
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-session-connection.md`));
  }
});
