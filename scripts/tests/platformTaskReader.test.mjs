// Wiring inventories supplement actual contract, browser and opt-in SQL tests.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const directory = "engine/src/bin/cyanrex-platform/session_ui";

test("private Task reading retains its two fixed assets inside the seven-asset optional shell", async () => {
  const source = await read(`${directory}.rs`);
  assert.equal([...source.matchAll(/\.route\(/g)].length, 7);
  for (const name of ["tasks.js", "task-contract.js"]) {
    assert.ok(source.includes(`.route("/platform/ui/${name}", any(dispatch))`));
    assert.ok(source.includes(`include_str!("session_ui/${name}")`));
  }
  assert.doesNotMatch(source, /std::(?:fs|env)|tokio::fs|ServeDir|PgPool|AuthService|AppState|reqwest/);
  const host = await read("engine/src/bin/cyanrex-platform/serve_loopback/mod.rs");
  assert.match(host, /if session_ui/);
  for (const file of ["engine/src/main.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(file), /createTaskReader|task-contract\.js|--session-ui/, file);
});

test("private task transport is explicit GET-only, same-origin and bounded independently from Session replies", async () => {
  const source = await read(`${directory}/tasks.js`);
  for (const value of ['method: "GET"', 'mode: "same-origin"', 'credentials: "same-origin"',
    'cache: "no-store"', 'redirect: "error"', '"X-Cyanrex-Platform-Request": "1"',
    "128 * 1024", "50 * 1024 * 1024", "16_384", "10_000", "Promise.race([work, interrupted])",
    'new TextDecoder("utf-8", { fatal: true })', "/platform/v1/task-index?limit=20",
    "/platform/v1/tasks/${entry.reference.task_id}", "parseIndex(response.raw", "await parseContent(response.raw"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /method:\s*["'](?:POST|PUT|PATCH|DELETE)|innerHTML|insertAdjacentHTML|DOMParser|localStorage|sessionStorage|indexedDB|document\.cookie|console\.|setInterval|location\.|\/api\/|TaskDraft/);
  assert.match(source, /load\.addEventListener\("click"/);
  assert.match(source, /next\.addEventListener\("click"/);
  assert.match(source, /text\.textContent = snapshot\.contents/);
});

test("current page and fresh authority observations fence the GET-only content reader", async () => {
  const source = await read(`${directory}/tasks.js`), app = await read(`${directory}/app.js`);
  for (const value of ["generation++", "controller?.abort()", "generation === epoch && isActive()",
    "clearDetail();", "clearList();", "if (!current() || rejectResponse(response)) return",
    "response.status === 401 || response.status === 403", "sessionRejected(response)",
    "deniedStatus = response.status", "failure.deniedStatus = deniedStatus", "error?.deniedStatus === 401"])
    assert.ok(source.includes(value), value);
  assert.match(app, /if \(state !== "active"\) taskReader\?\.clear\(\)/);
  assert.match(app, /taskReader\?\.setBusy\(value\)/);
  assert.match(app, /response\.status === 401 && invalidSession\(response\.raw\)/);
  assert.match(app, /mounted && status\.dataset\.state === "active"/);
  const html = await read(`${directory}/index.html`);
  for (const id of ["load-tasks", "next-tasks", "task-payload-select"])
    assert.match(html, new RegExp(`<[^>]+id="${id}"[^>]+disabled`));
  assert.match(html, /<pre id="task-text"/);
  assert.match(html, /id="task-content"[^>]+hidden/);
  assert.doesNotMatch(source + html, /Monaco|WebSocket|contenteditable|download=|Save Task|\/tasks\/new/);
});

test("pure Task wire checks retain duplicate, scope, revision, ordered pin and local digest boundaries", async () => {
  const source = await read(`${directory}/task-contract.js`);
  for (const value of ["keys.has(key)", "tokens > 8192", "depth > 16", "Number.isSafeInteger",
    "input.definition_ref !== null", "input.definition !== null",
    "result.revision < selected.revision", "result.revision === selected.revision",
    "same(result.input_refs, content.payload.map", "same(artifact, content.payload[index]?.artifact)",
    'crypto.subtle.digest("SHA-256", bytes)', "Object.freeze(contents)"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /fetch\(|localStorage|sessionStorage|document\.|console\.|AuthService/);
  const cases = await read("scripts/tests/platformTaskReaderContract.test.mjs");
  assert.ok([...cases.matchAll(/^test\(/gm)].length >= 20);
  assert.match(cases, /future-lang\+v2/);
  assert.match(cases, /32-item backslash content/);
  assert.match(cases, /unsafe or noncanonical numeric tokens/);
});

test("mock UI and a distinct real SQL/browser selector are exact opt-in CI selections", async () => {
  const browser = await read("scripts/tests/platformTasks.browser.mjs");
  assert.equal([...browser.matchAll(/^test\(/gm)].length, 12);
  const runner = await read("scripts/test-platform-browser-tasks.sh");
  const selector = "behavior::postgres_platform_browser_tasks_real_private_pages_reopen_and_revocation_never_write";
  for (const value of ["CYANREX_TEST_DATABASE_URL:?", "CYANREX_PLAYWRIGHT_MODULE:?", "CYANREX_CHROMIUM_PATH:?",
    selector, "--locked --test platform_browser_tasks_tdd", "--ignored --list", "grep -Fx", "--ignored --exact"])
    assert.ok(runner.includes(value), value);
  assert.doesNotMatch(runner, /initdb|pg_ctl|\bGRANT\b|start\.sh|\bDATABASE_URL=/);
  const behavior = await read("engine/tests/platform_browser_tasks/behavior.rs");
  assert.match(behavior, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
  for (const value of ["resources(f).await", "no_resource_writes(f).await", "p.base.suspend(0).await", "safe_threads", "stopped("])
    assert.ok(behavior.includes(value), value);
  const actual = await read("engine/tests/platform_browser_tasks/browser.mjs");
  assert.doesNotMatch(actual, /\.route\(|route\.fulfill|addCookies\(|ignoreHTTPSErrors:\s*true/);
  const bootstrap = await read("scripts/test-platform-browser-ci.sh");
  for (const name of ["test-platform-task-reader-browser.sh", "test-platform-browser-tasks.sh"])
    assert.ok(bootstrap.includes(`bash scripts/${name}`), name);
  assert.match(await read(".github/workflows/ci.yml"), /&& bash scripts\/test-platform-browser-ci\.sh/);
});

test("five-language read-only task guidance is current and bilingual host guides have course mirrors", async () => {
  const { labels } = await import(new URL(`${directory}/labels.js`, root));
  for (const language of ["en", "zh-CN", "es", "ja", "ko"]) {
    assert.deepEqual(Object.keys(labels[language]).sort(), Object.keys(labels.en).sort());
    for (const key of ["tasksTitle", "tasksReadOnly", "tasksEmptyPayload", "tasksUnknown", "taskStatus_in_review"])
      assert.ok(labels[language][key]?.trim(), `${language}:${key}`);
  }
  for (const language of ["en", "zh-CN"]) {
    for (const name of ["platform-loopback-host", "testing-guide"]) {
      const file = `${language}/${name}.md`, guide = await read(`docs/${file}`);
      for (const value of ["scripts/test-platform-browser-tasks.sh", "scripts/test-platform-task-reader-browser.sh"])
        assert.ok(guide.includes(value), `${file}:${value}`);
      assert.equal(guide, await read(`frontend/public/course/${file}`));
    }
  }
});
