// Source inventories complement pure contracts, mocked Chrome and original-storage browser SQL.
// Presence alone never establishes current authorization, persisted bytes or a confirmed COMMIT.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createObservations, readObservedJson } from "../../engine/tests/platform_browser_task_metadata/observations.mjs";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const directory = "engine/src/bin/cyanrex-platform/session_ui";

test("metadata fixture drain includes observations registered while earlier checks are pending", async () => {
  const observations = createObservations();
  let releaseHeader, releaseBody, bodyStarted, bodyChecked = false;
  const headers = new Promise(resolve => { releaseHeader = resolve; });
  const body = new Promise(resolve => { releaseBody = resolve; });
  const enteredBody = new Promise(resolve => { bodyStarted = resolve; });
  observations.track("request-headers", async () => {
    await headers;
    observations.track("reply-json", async () => {
      bodyStarted(); await body; bodyChecked = true;
    });
  });
  const drained = observations.drain();
  releaseHeader(); await enteredBody;
  try {
    const order = await Promise.race([drained.then(() => "early-drain"),
      new Promise(resolve => setImmediate(() => resolve("body-still-held")))]);
    assert.equal(order, "body-still-held");
  } finally { releaseBody(); await drained; }
  assert.equal(bodyChecked, true);
});

test("metadata fixture observation failures retain only fixed check codes without leaking private assertions", async () => {
  const observations = createObservations();
  observations.track("reply-save", async () => { throw new Error("private credentials and Task body"); });
  // Give a rejected observer a full turn before draining; it must not become unhandled.
  await new Promise(resolve => setImmediate(resolve));
  await assert.rejects(observations.drain(), error => error.message === "private browser observation failed"
    && error.check === "reply-save" && !JSON.stringify(error).includes("private credentials"));
});

test("metadata response observation distinguishes event, body and JSON failures without private error values", async () => {
  const cases = [
    [Promise.reject(Object.assign(new Error("private event detail"), { name: "TimeoutError" })), "event-timeout"],
    [Promise.reject(new Error("private event detail")), "event-failed"],
    [Promise.resolve({ text: async () => { throw new Error("Network.getResponseBody: No resource with given identifier found; private detail"); } }), "body-resource-missing"],
    [Promise.resolve({ text: async () => { throw new Error("Protocol error (Network.getResponseBody): private detail"); } }), "body-protocol"],
    [Promise.resolve({ text: async () => { throw new Error("private body detail"); } }), "body-failed"],
    [Promise.resolve({ text: async () => 'private invalid JSON' }), "json-invalid"],
  ];
  // Register every rejected event promise immediately, before awaiting any phase.
  const observations = cases.map(([incoming, part]) => {
    const checks = createObservations();
    checks.track("reply-save", () => readObservedJson(incoming));
    return { checks, part };
  });
  await new Promise(resolve => setImmediate(resolve));
  for (const { checks, part } of observations) {
    await assert.rejects(checks.drain(), error => error.check === `reply-save-${part}`
      && error.message === "private browser observation failed"
      && !JSON.stringify(error).includes("private detail"));
  }
});

test("metadata response observation reads one body and never retries a failed or malformed reply", async () => {
  let reads = 0;
  const response = { status: () => 200, text: async () => { reads++; return '{"schema_version":1}'; },
    json: () => assert.fail("body retrieval and JSON parsing must remain distinct") };
  assert.deepEqual(await readObservedJson(Promise.resolve(response)), { status: 200, body: { schema_version: 1 } });
  assert.equal(reads, 1);
  response.text = async () => { reads++; return ""; };
  const checks = createObservations();
  checks.track("reply-task", () => readObservedJson(Promise.resolve(response)));
  await assert.rejects(checks.drain(), error => error.check === "reply-task-json-invalid");
  assert.equal(reads, 2);
});

test("the separate metadata editor has one explicit same-origin PUT and no Artifact publication transport", async () => {
  const source = await read(`${directory}/task-edit.js`);
  assert.equal([...source.matchAll(/\bfetch\(/g)].length, 1);
  assert.match(source, /fetch\(`\/platform\/v1\/tasks\/\$\{descriptor\.task_id\}\/content`/);
  for (const value of ['method: "PUT"', 'mode: "same-origin"', 'credentials: "same-origin"',
    'cache: "no-store"', 'redirect: "error"', '"Content-Type": "application/json"',
    '"X-Cyanrex-Platform-Request": "1"', "body: JSON.stringify(descriptor.body)"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(source, /method:\s*["'](?:GET|POST|PATCH|DELETE)|\/platform\/v1\/(?:text-artifacts|artifacts)|localStorage|sessionStorage|indexedDB|document\.cookie|innerHTML|insertAdjacentHTML|console\.|setInterval|WebSocket|\/api\//);
  const reader = await read(`${directory}/tasks.js`);
  assert.match(reader, /import \{ createMetadataEditor \} from '\.\/task-edit\.js'/);
  assert.doesNotMatch(reader, /method:\s*["'](?:POST|PUT|PATCH|DELETE)/);
});

test("metadata preparation and acknowledgement retain independent wire, frame and waiting budgets", async () => {
  const contract = await read(`${directory}/task-contract.js`), source = await read(`${directory}/task-edit.js`);
  for (const value of ["export function prepareMetadataSave", "export function parseMetadataSaved",
    "items.length !== baseline.manifest.payload.length", "artifact: baseline.manifest.payload[index].artifact",
    "!same(content.payload.map(item => item.artifact), baseline.task.input_refs)",
    "same(content, baseline.manifest)", "encoder.encode(JSON.stringify(result)).length > 64 * 1024",
    'record(input.body,', '["schema_version", "expected_revision", "manifest"]',
    "record(decode(raw, INDEX_BYTES)", "const INDEX_BYTES = 128 * 1024",
    "result.revision !== expected.body.expected_revision + 1", "!same(content, expected.body.manifest)"])
    assert.ok(contract.includes(value), value);
  for (const value of ["++frames > 4096", "size > 128 * 1024", "10_000",
    "Promise.race([work, interrupted])", 'new TextDecoder("utf-8", { fatal: true })',
    "reader.releaseLock()", "clearTimeout(timer)"])
    assert.ok(source.includes(value), value);
  assert.doesNotMatch(contract, /fetch\(|document\.|localStorage|sessionStorage|AuthService/);
});

test("metadata confirmation fences the exact target and freezes terminal attempts without replay or hidden reread", async () => {
  const source = await read(`${directory}/task-edit.js`), html = await read(`${directory}/index.html`);
  for (const id of ["edit-task", "task-edit-title", "task-edit-item", "task-edit-filename", "task-edit-language",
    "save-task-metadata", "save-metadata-dialog", "metadata-save-target", "metadata-save-summary",
    "confirm-save-metadata", "cancel-save-metadata"])
    assert.ok(html.includes(`id="${id}"`), id);
  for (const value of ["const terminal = () => attempt !== null", "staged = { descriptor, generation, baseline }",
    "selected.generation !== generation || selected.baseline !== baseline", "attempt = selected.descriptor",
    'observe("pending", "metaPending")', 'byId("cancel-save-metadata").focus()',
    "parseMetadataSaved(response.raw, selected.baseline, selected.descriptor)",
    "contents: selected.baseline.contents", 'observe("saved", "metaSaved")',
    '"conflict" : "unknown"', "save.disabled = busy || !isActive() || quarantined || !editing || !dirty || terminal()"])
    assert.ok(source.includes(value), value);
  assert.ok(source.indexOf("attempt = selected.descriptor") < source.indexOf("await write("));
  assert.equal([...source.matchAll(/await write\(/g)].length, 1);
  assert.doesNotMatch(source, /\/platform\/v1\/task-index|location\.reload|requestSubmit|setInterval/);
});

test("quarantine clears private DOM while Cancel-focused leave and lifecycle generations protect held work", async () => {
  const source = await read(`${directory}/task-edit.js`), app = await read(`${directory}/app.js`);
  for (const value of ["generation++", "controller?.abort()", "closeDialogs(); emptyDom()",
    'title.value = ""; item.replaceChildren(); filename.value = ""; language.value = ""',
    "baseline = null; editing = false; quarantined = true", 'observe("quarantined", "metaQuarantined")',
    "acceptSnapshot(value) { if (held()) return", "departure = { generation, action }",
    "selected.generation !== generation", 'byId("cancel-task-leave").focus()',
    'leaving.addEventListener("cancel"', 'confirmation.addEventListener("cancel"',
    'window.addEventListener("beforeunload"', "event.preventDefault(); event.returnValue", "sessionRejected(response)"])
    assert.ok(source.includes(value), value);
  for (const value of ["taskReader?.hasWork()", "taskReader.leave(", 'taskReader.hasWork()',
    'window.addEventListener("pagehide"', 'window.addEventListener("pageshow"'])
    assert.ok(app.includes(value), value);
  const reader = await read(`${directory}/tasks.js`);
  for (const value of ["editor.invalidate()", "editor.leave(() => open(entry))", "editor.leave(() => loadPage())",
    "editor.leave(() => loadPage(cursor))", "editor.acceptSnapshot(value)"])
    assert.ok(reader.includes(value), value);
});

test("metadata is the seventh fixed opt-in asset with complete inert five-language controls", async () => {
  const source = await read(`${directory}.rs`), { labels } = await import(new URL(`${directory}/labels.js`, root));
  assert.deepEqual([...source.matchAll(/\.route\("([^"]+)"/g)].map(match => match[1]),
    ["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js", "/platform/ui/style.css",
      "/platform/ui/tasks.js", "/platform/ui/task-contract.js", "/platform/ui/task-edit.js"]);
  assert.ok(source.includes('include_str!("session_ui/task-edit.js")'));
  assert.doesNotMatch(source, /\.fallback\(|ServeDir|std::(?:fs|env)|tokio::fs|PgPool|AuthService|reqwest/);
  const keys = Object.keys(labels.en).filter(key => key.startsWith("meta"));
  assert.ok(keys.length >= 20);
  for (const language of ["en", "zh-CN", "es", "ja", "ko"]) {
    assert.deepEqual(Object.keys(labels[language]).sort(), Object.keys(labels.en).sort());
    for (const key of keys) assert.ok(labels[language][key]?.trim(), `${language}:${key}`);
  }
});

test("twelve mocked Chrome cases and one distinct original-storage selector are explicit CI runner selections", async () => {
  const browser = await read("scripts/tests/platformTaskMetadata.browser.mjs");
  assert.equal([...browser.matchAll(/^test\(/gm)].length, 12);
  assert.match(browser, /every API response is MOCKED/);
  const mockRunner = await read("scripts/test-platform-task-metadata-browser.sh");
  for (const value of ["CYANREX_PLAYWRIGHT_MODULE:?", "CYANREX_CHROMIUM_PATH:?",
    "node --test scripts/tests/platformTaskMetadata.browser.mjs"])
    assert.ok(mockRunner.includes(value), value);
  const runner = await read("scripts/test-platform-browser-task-metadata.sh");
  const selector = "behavior::postgres_platform_browser_task_metadata_confirmed_replace_conflict_and_revocation";
  for (const value of ["CYANREX_TEST_DATABASE_URL:?", "CYANREX_PLAYWRIGHT_MODULE:?", "CYANREX_CHROMIUM_PATH:?",
    selector, "--locked --test platform_browser_task_metadata_tdd", "--ignored --list", "grep -Fx", "--ignored --exact"])
    assert.ok(runner.includes(value), value);
  assert.doesNotMatch(runner, /initdb|pg_ctl|\bGRANT\b|start\.sh|\bDATABASE_URL=/);
  const behavior = await read("engine/tests/platform_browser_task_metadata/behavior.rs");
  assert.equal([...behavior.matchAll(/#\[tokio::test\]/g)].length, 1);
  assert.ok(behavior.includes(`async fn ${selector.split("::")[1]}()`));
  for (const value of ["requires an explicit disposable CYANREX_TEST_DATABASE_URL", "saved_manifest(&original)",
    "p.admin_advance(&saved).await", "revoke(f, &digest).await", "immutable_rows(f).await", "p.files()",
    "safe_threads", "stopped(", "no_connections(f).await", "p.finish()"])
    assert.ok(behavior.includes(value), value);
  const actual = await read("engine/tests/platform_browser_task_metadata/browser.mjs");
  assert.match(actual, /ignoreHTTPSErrors: false/);
  assert.doesNotMatch(actual, /\.route\(|route\.fulfill|addCookies\(|ignoreHTTPSErrors:\s*true/);
  for (const value of ['import { createObservations, readObservedJson } from "./observations.mjs"',
    "page.waitForResponse", 'response.request().method() === method', 'response.url() === `${origin}${path}`',
    'expectReply(taskPath, "GET")', 'expectReply(savePath, "PUT")', "await observations.drain()",
    "...await readObservedJson(incoming)",
    'stage = "put-response-observed"', 'stage = "saved-rendered-title"',
    '"browser-wait-timeout"', 'emit({ phase: "failure", stage, check })'])
    assert.ok(actual.includes(value), value);
  assert.equal([...actual.matchAll(/expectReply\(savePath, "PUT"\)/g)].length, 3);
  assert.doesNotMatch(actual, /emit\([^\n]*(?:error\.message|error\.stack|reply\.body|config)/);
  const bootstrap = await read("scripts/test-platform-browser-ci.sh");
  assert.ok(bootstrap.includes("Platform browser runtime: Node %s; %s"));
  const runners = ["session-ui-browser", "task-reader-browser", "task-metadata-browser",
    "browser-session", "browser-tasks", "browser-task-metadata"].map(name => `test-platform-${name}.sh`);
  assert.deepEqual([...bootstrap.matchAll(/^bash scripts\/(test-platform-[\w-]+\.sh)$/gm)].map(match => match[1]).sort(), runners.sort());
  assert.match(await read(".github/workflows/ci.yml"), /&& bash scripts\/test-platform-browser-ci\.sh/);
});
