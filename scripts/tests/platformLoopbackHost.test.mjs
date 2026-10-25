import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const strip = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => strip(source).replace(/\s+/g, "");
const directory = "engine/src/bin/cyanrex-platform/serve_loopback";

async function productSources() {
  const entries = await readdir(new URL(`${directory}/`, root), { recursive: true });
  // Notification test helpers are siblings of tests.rs, not below tests/. Exclude
  // only files explicitly owned by its verified cfg(test)-only module tree.
  const testOnly = new Set();
  if (entries.includes("notify/tests.rs")) {
    const notify = await read(`${directory}/notify.rs`);
    assert.match(notify, /#\[cfg\(test\)\]\s*#\[path = "notify\/tests\.rs"\]\s*mod tests;/);
    assert.doesNotMatch(strip(notify), /\bmod\s+(?!tests\b)\w+\s*;/);
    for (const [, file] of (await read(`${directory}/notify/tests.rs`))
      .matchAll(/#\[path = "(\w+\.rs)"\]\s*mod \w+;/g))
      testOnly.add(`notify/${file}`);
  }
  const files = entries.filter(path => path.endsWith(".rs") && !testOnly.has(path)
    && !/(^|\/)tests(?:\.rs|\/)/.test(path));
  assert.ok(files.includes("mod.rs") && files.includes("http.rs") && files.includes("request.rs"));
  return (await Promise.all(files.map(file => read(`${directory}/${file}`)))).join("\n");
}

// Inventories supplement actual CLI, TCP and PostgreSQL behavior; they are not isolation proof.
test("explicit loopback mode keeps main thin and never composes Engine or a diagnostic baseline", async () => {
  const main = await read("engine/src/bin/cyanrex-platform/main.rs");
  for (const value of ["take(17)", "7 | 9 | 11 | 12 | 15 | 16", '"serve-loopback"', '"--notify-systemd"', 'text[9] == "--bind"',
    "serve_loopback::parse_bind", "serve_loopback::run", "platform loopback host failed",
    "platform loopback host stopped"]) assert.ok(main.includes(value), value);
  const usage = main.match(/"(usage: cyanrex-platform[^"\n]+)"/)?.[1];
  assert.ok(usage && Buffer.byteLength(`${usage}\n`) <= 256, "preserve the original bounded usage response");
  assert.doesNotMatch(strip(main), /tokio|TcpListener|PgPool|PgConnectOptions|open_platform_storage_expected|build_platform_(?:session|workspace)_router/);
  const source = strip(await productSources());
  assert.doesNotMatch(source, /\b(?:Engine|AppState|AuthService|DurableAuthSource|PgPool|PgConnectOptions)\b|dotenv|set_var|remove_var|storage_check::|check_platform_database_storage|install_(?!panic_hook)|migrate|adopt_|axum::serve|with_upgrades|http2/);
  // Only the separately guarded, explicit supervisor adapter may read one environment key.
  const withoutNotify = (await productSources()).replace(await read(`${directory}/notify.rs`), "");
  assert.doesNotMatch(strip(withoutNotify), /std::env|\benv::/);
  assert.doesNotMatch(source, /pub\s+(?:async\s+)?fn/);
  assert.equal([...source.matchAll(/open_platform_storage_expected\(/g)].length, 1);
  assert.equal([...source.matchAll(/PlatformWorkspaceHttpState::new\(/g)].length, 1);
  assert.doesNotMatch(source, /PlatformSessionHttpState::new\(|build_platform_session_router\(/);
  assert.match(source, /std::panic::set_hook\(Box::new\(\|_\| eprintln!\("platform loopback host failed"\)\)\)/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /serve_loopback|serve-loopback|build_platform_(?:session|workspace)_router/, path);
});

test("literal bind policy and startup require original complete storage before listening", async () => {
  const source = await productSources();
  const code = compact(source);
  for (const value of ["bind.port()<1024", "Ipv4Addr::LOCALHOST", "Ipv6Addr::LOCALHOST",
    "address.scope_id()!=0", "bind.to_string()!=value", "StorageExpectation::Markers",
    "config::load(", "verify_before_runtime()", "verify_current_thread()",
    "new_current_thread()", ".enable_io()", ".enable_time()", "open_platform_storage_expected(",
    "build_platform_workspace_router(", "SignalKind::terminate()", "SignalKind::interrupt()",
    "TcpListener::bind(", "shutdown_timeout(Duration::from_secs(1))"])
    assert.ok(code.includes(value), value);
  assert.ok(source.includes('let scheme = if tls_paths.is_some() { "https" } else { "http" };'));
  assert.ok(source.includes('input.config.origin() != format!("{scheme}://{bind}")'));
  assert.ok(source.includes("platform loopback host ready"));
  assert.doesNotMatch(code, /new_multi_thread|TcpSocket|lookup_host|to_socket_addrs|retry|\.port\(0\)/);
});

test("transport caps admission before spawning and leaves dispatched work outside timeouts", async () => {
  const http = compact(await read(`${directory}/http.rs`));
  for (const value of ["CONNECTIONS:usize=16", "HEADER_TIMEOUT:Duration=Duration::from_secs(2)",
    "DRAIN_TIMEOUT:Duration=Duration::from_secs(30)", "try_join_next()", "try_acquire_owned()",
    "connections.spawn(", "TowerToHyperService::new(router)", "TokioIo::new(stream)",
    ".timer(TokioTimer::new())", ".max_headers(32)", ".max_buf_size(16*1024)",
    ".header_read_timeout(HEADER_TIMEOUT)", ".keep_alive(false)", "OwnedSemaphorePermit",
    "graceful_shutdown()", "drop(listener)", "connections.abort_all()"])
    assert.ok(http.includes(value), value);
  assert.ok(http.indexOf("try_acquire_owned()") < http.indexOf("connections.spawn("));
  assert.equal([...http.matchAll(/tokio::time::timeout\(/g)].length, 1, "only shutdown drain gets an outer timeout");
  assert.ok(http.includes("tokio::time::timeout(DRAIN_TIMEOUT,"));
  const gate = compact(await read(`${directory}/request.rs`));
  for (const value of ["get_all(header::HOST)", "authority.as_bytes()", "hosts.next().is_none()",
    "Version::HTTP_11", "Method::CONNECT", "header::UPGRADE", "uri.scheme().is_some()",
    "uri.authority().is_some()", "Body::empty()", "StatusCode::BAD_REQUEST",
    "header::CACHE_CONTROL", "header::PRAGMA", "header::X_CONTENT_TYPE_OPTIONS"])
    assert.ok(gate.includes(value), value);
  assert.doesNotMatch(gate, /SET_COOKIE|LOCATION|ACCESS_CONTROL|FORWARDED|\.uri_mut\(|request\.headers_mut\(/);
  const manifest = await read("engine/Cargo.toml");
  assert.match(manifest, /^hyper = .*features = \["server", "http1"\]/m);
  assert.match(manifest, /^hyper-util = .*features = \["tokio", "service"\]/m);
});

test("every real host SQL case is selected exactly and bilingual scope stays explicit", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_loopback_host/", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_loopback_host/${file}`))
      .matchAll(/async fn (postgres_platform_loopback_host_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 8, "new SQL boundaries require deliberate exact runner coverage");
  const runner = await read("scripts/test-platform-loopback-host.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_loopback_host_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_loopback_host_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  assert.match(await read(".github/workflows/ci.yml"), /&& bash scripts\/test-platform-loopback-host\.sh/);
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-loopback-host.md`);
    for (const value of ["issues/3", "serve-loopback", "127.0.0.1", "[::1]", "1024", "65535",
      "PGOPTIONS", "16 KiB", "SIGTERM", "SIGINT", "32", "30", "platform loopback host ready",
      "platform-session-router.md", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /Cookies are not isolated by port/ : /Cookie 也不按端口/);
    assert.match(guide, locale === "en" ? /no hard process\s+exit deadline/ : /不保证硬性进程退出期限/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-loopback-host.md`));
    for (const file of ["README.md", "architecture-map.md", "architecture.md", "platform-network.md", "capability-maturity.md", "testing-guide.md", "platform-session-router.md",
      "platform-storage-open.md", "platform-login-http.md", "platform-process-startup.md", "platform-startup-input.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-loopback-host.md`);
  }
});
