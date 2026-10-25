import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory, extension) {
  const output = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) output.push(...await sources(path, extension));
    else if (extension.test(entry.name)) output.push([path, await read(path)]);
  }
  return output;
}

// Source inventories are not proof of SQL authorization, cookie delivery or a mounted platform.
test("session lifecycle remains explicitly exported but absent from the live application and SDK", async () => {
  const module = await read("engine/src/platform_http/mod.rs");
  assert.match(module, /pub use session_lifecycle::\{build_session_lifecycle_router, SessionLifecycleHttpState\}/);
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs",
    "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  for (const [directory, extension] of [["sdk-js/src", /\.ts$/], ["frontend/src/features/tasks", /\.tsx?$/]]) {
    files.push(...await sources(directory, extension));
  }
  for (const [path, source] of files) {
    assert.doesNotMatch(codeOnly(source), /\b(?:SessionLifecycleHttpState|build_session_lifecycle_router)\b/, path);
    // Preserve URL string contents: a // stripper can hide endpoints after "https:".
    assert.doesNotMatch(source, /\/platform\/v1\/session/, path);
  }
});

test("session lifecycle delegates to original source primitives without issuing or installing", async () => {
  const files = await sources("engine/src/platform_http/session_lifecycle", /\.rs$/);
  const code = files.map(([, source]) => codeOnly(source)).join("\n");
  assert.match(code, /\.validate_session\(\s*&token\s*\)/);
  assert.match(code, /\.logout\(\s*&token\s*\)/);
  assert.match(code, /security::credentials\(/);
  assert.match(code, /security::request_bytes\(/);
  assert.match(code, /admission::acquire\(/);
  assert.match(code, /admission::response\(/);
  assert.doesNotMatch(code, /\.(?:login|register|install_empty_schema|prune_expired_sessions)\s*\(|AuthService|std::env|tokio::spawn|spawn_blocking|build_state|TcpListener/);
});

test("session lifecycle documents fixed cookie clearing and the still separate issuer host and Task workflow", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const document = await read(`docs/${locale}/platform-session-lifecycle.md`);
    for (const value of ["/platform/v1/session", "/platform/v1/session/logout", "cyanrex_platform_session",
      "Path=/platform/v1", "SameSite=Strict", "Max-Age=0", "HttpOnly", "Secure", "issues/3"]) assert.ok(document.includes(value), value);
    assert.match(document, locale === "en" ? /unmounted/i : /未挂载/);
    assert.match(document, locale === "en" ? /unknown|unconfirmed/i : /不确定|未确认/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-session-lifecycle.md`);
  }
});

test("session lifecycle default and disposable SQL regressions have a deliberate exact runner", async () => {
  const main = await read("engine/tests/platform_session_http_tdd.rs");
  assert.ok(main.includes("platform_session_http/transport.rs"));
  const sources = await read("engine/tests/platform_session_http/behavior.rs");
  assert.match(sources, /#\[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"\]/);
  const runner = await read("scripts/test-platform-session-http.sh");
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test platform_session_http_tdd/);
  assert.match(runner, /--ignored --exact/);
  assert.doesNotMatch(runner, /DATABASE_URL=|install|initdb|pg_ctl|sudo|start\.sh/);
});
