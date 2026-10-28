// Source inventories complement, but never replace, real router/browser/SQL evidence.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const ui = "engine/src/bin/cyanrex-platform/session_ui";

test("Session page is a final explicit CLI flag merged only inside the existing private host", async () => {
  const cli = await read("engine/src/bin/cyanrex-platform/main.rs");
  assert.match(cli, /take\(19\)/);
  assert.match(cli, /text\.last\(\) == Some\(&"--session-ui"\)/);
  assert.match(cli, /without_ui - 1/);
  assert.match(cli, /\[--notify-systemd\] \[--session-ui\]/);
  const host = await read("engine/src/bin/cyanrex-platform/serve_loopback/mod.rs");
  assert.match(host, /let router = if session_ui/);
  assert.match(host, /router\.merge\(super::session_ui::routes\(&format!\("\{scheme\}:\/\/\{bind\}"\)\)\)/);
  const merge = host.indexOf("let router = if session_ui");
  assert.ok(host.indexOf("open_platform_storage_expected(") < merge);
  assert.ok(merge < host.indexOf("TcpListener::bind("));
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(file), /session_ui::routes|--session-ui/, file);
});

test("closed compile-time static assets have no directory, fallback, storage, proxy or teaching integration", async () => {
  const source = await read(`${ui}.rs`);
  assert.deepEqual([...source.matchAll(/\.route\("([^"]+)"/g)].map(match => match[1]),
    ["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js", "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js", "/platform/ui/task-edit.js"]);
  assert.deepEqual([...source.matchAll(/include_str!\("session_ui\/([^"]+)"\)/g)].map(match => match[1]),
    ["index.html", "app.js", "labels.js", "style.css", "tasks.js", "task-contract.js", "task-edit.js"]);
  assert.doesNotMatch(source, /\.fallback\(|ServeDir|std::(?:fs|env)|tokio::fs|PgPool|AppState|AuthService|reqwest|SetCookie/);
  for (const value of ["form-action 'none'", "frame-ancestors 'none'", "connect-src 'self'",
    "base-uri 'none'", "object-src 'none'", "worker-src 'none'", "no-store", "no-referrer", "nosniff"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /access-control-allow|strict-transport-security/i);
});

test("static requests reject ambiguity before bounded body collection without treating metadata as auth", async () => {
  const source = await read(`${ui}.rs`);
  for (const value of ["request.method() != Method::GET", "request.uri().query().is_some()",
    "header::AUTHORIZATION", 'optional_single(headers, "origin")', 'optional_single(headers, "sec-fetch-site")',
    'optional_single(headers, "sec-fetch-dest")', 'optional_single(headers, "sec-fetch-mode")',
    "header::CONTENT_ENCODING", "header::CONTENT_TYPE", "Duration::from_secs(2)", "tokio::task::yield_now()",
    "Method::HEAD", "Body::empty()", 'HeaderValue::from_static("0")']) assert.ok(source.includes(value), value);
  assert.ok(source.indexOf("preflight(request.headers()") < source.indexOf("empty_body(request.into_body())"));
  const tests = await read(`${ui}/tests.rs`);
  assert.equal([...tests.matchAll(/#\[tokio::test\]/g)].length, 15);
  const cli = await read("engine/tests/platform_session_ui_defaults_tdd.rs");
  assert.equal([...cli.matchAll(/#\[test\]/g)].length, 3);
  for (const value of ["private_fifo_open", "wrong_identity", "notify_then_ui"]) assert.ok(cli.includes(value), value);
});

test("browser script uses only three same-origin Session APIs and keeps ambiguous outcomes unknown", async () => {
  const app = await read(`${ui}/app.js`);
  assert.deepEqual([...app.matchAll(/request\("([^"]+)"/g)].map(match => match[1]),
    ["/platform/v1/session", "/platform/v1/session/login", "/platform/v1/session/logout"]);
  for (const value of ['mode: "same-origin"', 'credentials: "same-origin"', 'cache: "no-store"',
    'redirect: "error"', '"X-Cyanrex-Platform-Request": "1"', 'observe("unknown", "unknown")',
    "Promise.race([work, interrupted])", "10_000", "frames > 4096", "length > 4096",
    'new TextDecoder("utf-8", { fatal: true })', "finally { handedOff(); }",
    'window.addEventListener("pagehide"', 'window.addEventListener("pageshow"',
    "generation++", "if (!event.persisted) return", "mounted && generation === epoch",
    'element("cancel-logout").focus()', "if (busy || !mounted || !dialog.open) return"])
    assert.ok(app.includes(value), value);
  assert.doesNotMatch(app, /localStorage|sessionStorage|document\.cookie|indexedDB|console\.|setInterval|NEXT_PUBLIC|\/api\/|authorization\s*:/i);
  const html = await read(`${ui}/index.html`);
  assert.match(html, /method="post" action="\/platform\/v1\/session\/login"/);
  assert.match(html, /<script type="module" src="\/platform\/ui\/app\.js"><\/script>/);
  assert.doesNotMatch(html, /<script(?![^>]*\bsrc=)|\son(?:click|submit|error)=|<iframe|<base\b/);
  for (const id of ["username", "password", "otp", "login", "check-session", "logout"])
    assert.match(html, new RegExp(`<[^>]+id="${id}"[^>]+disabled`));
});

test("real Chrome UI behavior is explicit opt-in and clearly separate from actual SQL acceptance", async () => {
  const browser = await read("scripts/tests/platformSessionUi.browser.mjs");
  assert.equal([...browser.matchAll(/^test\(/gm)].length, 12);
  assert.match(browser, /explicitly mocked/);
  for (const value of ["pagehide", "pageshow", "5000", '"en", "zh-CN", "es", "ja", "ko"', "width: 360"])
    assert.ok(browser.includes(value), value);
  const helper = await read("scripts/tests/platformSessionUi/browserFixture.mjs");
  assert.match(helper, /MOCK Session APIs/);
  assert.match(helper, /read the actual fixed Rust CSP as data/);
  const runner = await read("scripts/test-platform-session-ui-browser.sh");
  for (const value of ["CYANREX_PLAYWRIGHT_MODULE:?", "CYANREX_CHROMIUM_PATH:?",
    "node --test scripts/tests/platformSessionUi.browser.mjs"]) assert.ok(runner.includes(value), value);
});

test("two actual SQL browser selectors and CI preserve ordinary TLS verification and private trust", async () => {
  const runner = await read("scripts/test-platform-browser-session.sh");
  const selectors = [...runner.matchAll(/^  (\w+::postgres_platform_browser_\w+)\b/gm)].map(match => match[1]);
  assert.equal(selectors.length, 2); assert.equal(new Set(selectors).size, 2);
  for (const selector of selectors) {
    const [file, name] = selector.split("::");
    const testSource = await read(`engine/tests/platform_browser_session/${file}.rs`);
    assert.match(testSource, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
    assert.ok(testSource.includes(`async fn ${name}()`));
  }
  for (const value of ["--locked --test platform_browser_session_tdd", "--ignored --list", "grep -Fx", "--ignored --exact"])
    assert.ok(runner.includes(value), value);
  const browser = await read("engine/tests/platform_browser_session/browser.mjs");
  assert.match(browser, /ignoreHTTPSErrors: false/);
  assert.match(browser, /ERR_CERT_AUTHORITY_INVALID/);
  assert.match(browser, /XDG_DATA_HOME: dataDirectory/);
  assert.match(browser, /legacy user NSS/);
  assert.doesNotMatch(browser, /ignore-certificate-errors|ignoreHTTPSErrors:\s*true|\.route\(|addCookies\(/);
  const bootstrap = await read("scripts/test-platform-browser-ci.sh");
  for (const value of ["CI:?", "RUNNER_TEMP:?", "mktemp -d", "playwright@1.62.1",
    "libnss3-tools", "bash scripts/test-platform-session-ui-browser.sh", "bash scripts/test-platform-browser-session.sh"])
    assert.ok(bootstrap.includes(value), value);
  assert.doesNotMatch(bootstrap, /(?:^|\s)(?:export\s+)?(?:HOME|home|CODEX_HOME)=|ignore-certificate-errors/);
  assert.match(await read(".github/workflows/ci.yml"), /&& bash scripts\/test-platform-browser-ci\.sh/);
});

test("bilingual host guides expose the optional session boundary with exact course mirrors", async () => {
  for (const language of ["en", "zh-CN"]) for (const name of ["platform-loopback-host", "platform-loopback-tls-host"]) {
    const file = `${language}/${name}.md`, guide = await read(`docs/${file}`);
    for (const value of ["--session-ui", "/platform/", "scripts/test-platform-browser-session.sh"])
      assert.ok(guide.includes(value), `${file}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${file}`));
  }
});
