// Real host, natural HttpOnly Cookie and passive CDP. SQL owner row locks coordinate the
// independent parent; no fulfilled responses, request replay, injected fetch or product hook.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, isAbsolute } from "node:path";
import { pathToFileURL } from "node:url";
import { createInterface } from "node:readline";
import { createObservations } from "../platform_browser_task_metadata/observations.mjs";
import { createCdpResponses } from "../platform_browser_task_metadata/cdp-responses.mjs";

const lines = createInterface({ input: process.stdin, terminal: false });
const input = lines[Symbol.asyncIterator]();
const read = async () => {
  const next = await input.next();
  assert.ok(!next.done && Buffer.byteLength(next.value) < 16 * 1024, "bounded private IPC required");
  return JSON.parse(next.value);
};
const emit = value => process.stdout.write(`${JSON.stringify(value)}\n`);
const advance = async () => assert.deepEqual(await read(), { continue: true });
const normalize = value => value.replace(/\r\n?/g, "\n");
const hash = value => createHash("sha256").update(value, "utf8").digest("hex");
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;
const policy = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src data:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'";
const assets = new Set(["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js",
  "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js",
  "/platform/ui/task-edit.js", "/platform/ui/request.js", "/platform/ui/body-contract.js",
  "/platform/ui/task-save/taskContentSave.js", "/platform/ui/task-save/taskSaveContract.js",
  "/platform/ui/task-save/taskDraft.js", "/platform/ui/task-save/taskPayloadLanguage.js"]);
let context, directory, stage = "partial-browser-startup";

async function run(config) {
  assert.equal(config.tls, false, "partial-prefix selectors are independently plain HTTP");
  assert.equal(typeof config.revoked, "boolean");
  const origin = new URL(config.origin).origin;
  assert.equal(new URL(origin).protocol, "http:");
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"])
    assert.ok(process.env[key] && isAbsolute(process.env[key]), "explicit browser dependency required");
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  directory = await fs.mkdtemp(join(tmpdir(), "cyanrex-platform-browser-body-prefix-"));
  await fs.chmod(directory, 0o700);
  context = await chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    viewport: { width: 1100, height: 1000 }, env: { ...process.env,
      XDG_DATA_HOME: join(directory, "data"), XDG_CONFIG_HOME: join(directory, "config"),
      XDG_CACHE_HOME: join(directory, "cache") },
  });
  const page = context.pages()[0], calls = [], errors = [];
  const observations = createObservations();
  const responses = await createCdpResponses(await context.newCDPSession(page));
  const taskPath = `/platform/v1/tasks/${config.text_id}`;
  const publishPath = `${taskPath}/text-artifacts`, savePath = `${taskPath}/content`;
  let privateToken = null, secondRequest;
  const secondRequested = new Promise(resolve => { secondRequest = resolve; });
  page.on("pageerror", () => errors.push("unexpected partial-prefix browser runtime error"));
  page.on("request", request => {
    const url = new URL(request.url()), method = request.method();
    const record = { path: url.pathname, method, query: url.search }; calls.push(record);
    const checked = (async () => {
      assert.equal(url.origin, origin); assert.ok(!url.username && !url.password && !url.hash);
      if (url.pathname === publishPath) {
        // Provisional request body/method are available now. Playwright's allHeaders()
        // can await response-bound ExtraInfo, so it cannot coordinate an in-flight SQL fence.
        record.body = JSON.parse(request.postData());
        if (calls.filter(value => value.path === publishPath).length === 2) secondRequest();
      }
      const headers = await request.allHeaders(); assert.equal(headers.authorization, undefined);
      if (assets.has(url.pathname) || url.pathname === "/favicon.ico") {
        assert.equal(method, "GET"); assert.equal(url.search, ""); assert.equal(headers.cookie, undefined); return;
      }
      assert.equal(headers["x-cyanrex-platform-request"], "1");
      assert.equal(headers["sec-fetch-mode"], "same-origin");
      assert.equal(headers["sec-fetch-site"], "same-origin");
      if (url.pathname === "/platform/v1/session/login") {
        assert.equal(method, "POST"); assert.equal(headers.origin, origin); assert.equal(url.search, ""); return;
      }
      if (url.pathname === publishPath) {
        assert.equal(method, "POST"); assert.equal(url.search, ""); assert.equal(headers.origin, origin);
        assert.equal(headers.cookie, `cyanrex_platform_session=${privateToken}`);
        assert.match(headers["content-type"], /^application\/json(?:;\s*charset=utf-8)?$/i);
        return;
      }
      assert.equal(method, "GET", "no Task PUT, unbound write, logout or replay is authorized");
      if (url.pathname === "/platform/v1/session") { assert.equal(url.search, ""); return; }
      if (url.pathname === "/platform/v1/task-index") {
        assert.deepEqual([...url.searchParams], [["limit", "20"]]); return;
      }
      assert.equal(url.pathname, taskPath); assert.equal(url.search, "");
    })();
    checked.catch(() => {});
    observations.track("request-contract", () => checked);
  });
  page.on("response", response => {
    const path = new URL(response.url()).pathname;
    if (!assets.has(path)) return;
    observations.track("reply-asset", async () => {
      const headers = await response.allHeaders(); assert.equal(response.status(), 200);
      assert.equal(headers["cache-control"], "no-store"); assert.equal(headers["content-security-policy"], policy);
      assert.equal(headers["x-content-type-options"], "nosniff"); assert.equal(headers["referrer-policy"], "no-referrer");
      assert.equal(headers["cross-origin-resource-policy"], "same-origin");
      assert.equal(headers["cross-origin-opener-policy"], "same-origin");
      assert.equal(headers["x-frame-options"], "DENY"); assert.equal(headers["set-cookie"], undefined);
    });
  });
  const posts = () => calls.filter(call => call.path === publishPath && call.method === "POST");
  const puts = () => calls.filter(call => call.path === savePath && call.method === "PUT");
  const reads = () => calls.filter(call => call.method === "GET" && call.path.startsWith("/platform/v1/")).length;
  const flush = async () => {
    observations.track("reply-publish", () => responses.check());
    await observations.drain(); assert.deepEqual(errors, []);
  };
  const state = async (id, expected) => {
    stage = `${id}-${expected}`;
    await page.waitForFunction(({ id, expected }) => document.getElementById(id)?.dataset.state === expected
      && document.body.dataset.busy === "false", { id, expected }, { timeout: 10000 });
    await flush();
  };
  const fields = async () => ({ title: await page.locator("#task-edit-title").inputValue(),
    filename: await page.locator("#task-edit-filename").inputValue(),
    language: await page.locator("#task-edit-language").inputValue(),
    text: await page.locator("#task-edit-text").inputValue() });
  const clean = async () => {
    const stored = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    assert.ok(![privateToken, config.password, config.otp, config.first_text, config.second_text,
      config.text_title, config.saved_title, ...config.saved_names, ...config.saved_texts]
      .filter(Boolean).some(value => stored.includes(value)), "no private Web Storage persistence");
    assert.ok(!(await page.locator("body").textContent()).includes(privateToken));
    assert.ok(!(await page.locator("body").textContent()).includes(config.foreign_title));
    assert.ok(!(await page.evaluate(() => document.cookie)).includes("cyanrex_platform_session"));
    assert.equal(page.frames().length, 1); assert.equal(context.pages().length, 1);
  };
  await page.goto(`${origin}/platform/`); await state("status", "unauthenticated");
  assert.equal(await page.locator("body").getAttribute("data-task-text-publication"), "enabled");
  assert.equal(reads(), 1); assert.ok(!Object.hasOwn(config, "otp")); emit({ phase: "ready_login" });
  const fresh = await read(); assert.deepEqual(Object.keys(fresh), ["otp"]); assert.match(fresh.otp, /^[0-9]{6}$/);
  config.otp = fresh.otp;
  await page.locator("#username").fill(config.username); await page.locator("#password").fill(config.password);
  await page.locator("#otp").fill(config.otp); await page.locator("#login").click(); await state("status", "active");
  const cookies = (await context.cookies(`${origin}/platform/v1/session`)).filter(value => value.name === "cyanrex_platform_session");
  assert.equal(cookies.length, 1); const cookie = cookies[0]; privateToken = cookie.value;
  assert.match(privateToken, uuid); assert.equal(cookie.domain, new URL(origin).hostname);
  assert.equal(cookie.path, "/platform/v1"); assert.equal(cookie.httpOnly, true);
  assert.equal(cookie.sameSite, "Strict"); assert.equal(cookie.secure, false); assert.equal(cookie.expires, -1);
  assert.equal(await page.locator("#password").inputValue(), ""); assert.equal(await page.locator("#otp").inputValue(), "");
  await clean(); emit({ phase: "authenticated", digest: hash(privateToken) }); await advance();
  await page.locator("#load-tasks").click(); await state("task-read-state", "listed");
  const opened = responses.expect(`${origin}${taskPath}`, "GET");
  observations.track("reply-task", async () => { await opened; });
  await page.locator(`#task-list button[data-task-id="${config.text_id}"]`).click(); await state("task-read-state", "opened");
  const readReply = await opened; assert.equal(readReply.status, 200); const baseline = readReply.body;
  assert.equal(baseline.task.revision, 1); assert.equal(baseline.task.title, config.text_title);
  assert.equal(baseline.manifest.payload.length, 2);
  assert.equal(await page.locator("#task-text").textContent(), config.first_text);
  await page.locator("#edit-task").click(); await state("task-edit-state", "clean");
  await page.locator("#task-edit-title").fill(config.saved_title);
  for (let index = 0; index < 2; index++) {
    await page.locator("#task-edit-item").selectOption({ index });
    await page.locator("#task-edit-filename").fill(config.saved_names[index]);
    await page.locator("#task-edit-language").fill(config.saved_languages[index]);
    await page.locator("#task-edit-text").fill(normalize(config.saved_texts[index]));
  }
  await page.locator("#task-edit-item").selectOption({ index: 0 }); await state("task-edit-state", "dirty");
  const draft = await fields(), readsBeforeSave = reads();
  await page.locator("#save-task-metadata").click(); await page.locator("#save-metadata-dialog").waitFor({ state: "visible" });
  await page.waitForFunction(() => document.activeElement?.id === "cancel-save-metadata", null, { timeout: 10000 });
  assert.ok((await page.locator("#metadata-save-target").textContent()).includes(config.text_id));
  assert.equal(posts().length, 0); assert.equal(puts().length, 0); await flush();
  emit({ phase: "prepared" }); await advance();

  // Both same-URL requests are armed before the one explicit confirmation. The passive
  // observer reads each actual response body once; it never releases or delays the app.
  const incoming = responses.expectMany(`${origin}${publishPath}`, "POST", 2);
  incoming.forEach(reply => observations.track("reply-publish", async () => { await reply; }));
  stage = "first-committed-prefix-pending-second";
  await page.locator("#confirm-save-metadata").click();
  await page.waitForFunction(() => document.getElementById("task-save-progress")?.dataset.confirmedArtifacts === "1"
    && document.getElementById("task-save-progress")?.dataset.taskConfirmed === "false"
    && document.getElementById("task-edit-state")?.dataset.state === "pending"
    && document.body.dataset.busy === "true", null, { timeout: 10000 });
  const first = await incoming[0]; await secondRequested;
  // Do not await header checks or drain the ledger: the second response is deliberately
  // in flight, and allHeaders may need that response. Every check is retained for final drain.
  assert.deepEqual(errors, []);
  assert.equal(posts().length, 2); assert.equal(puts().length, 0); assert.equal(reads(), readsBeforeSave);
  for (let index = 0; index < 2; index++) {
    const body = posts()[index].body;
    assert.deepEqual(Object.keys(body).sort(), ["artifact_id", "expected", "revision_id", "schema_version", "text"]);
    assert.equal(body.schema_version, 1); assert.equal(body.text, config.saved_texts[index]);
    assert.deepEqual(body.expected, { task: baseline.task, manifest: baseline.manifest });
    assert.match(body.artifact_id, uuid); assert.match(body.revision_id, uuid);
    assert.ok(!baseline.task.input_refs.some(pin => pin.artifact_id === body.artifact_id || pin.revision_id === body.revision_id));
  }
  assert.equal(first.status, 201); assert.deepEqual(Object.keys(first.body).sort(), ["artifact", "schema_version"]);
  assert.equal(first.body.schema_version, 1);
  const artifact = first.body.artifact;
  assert.deepEqual(Object.keys(artifact).sort(), ["byte_length", "created_at", "kind", "media_type", "owner", "parent", "reference", "schema_version", "sequence", "title"]);
  assert.equal(artifact.schema_version, 1); assert.equal(artifact.sequence, 1); assert.equal(artifact.parent, null);
  assert.equal(artifact.title, baseline.task.title); assert.deepEqual(artifact.owner, baseline.task.owner);
  assert.equal(artifact.kind, "cyanrex.task-text"); assert.equal(artifact.media_type, "text/plain");
  assert.equal(artifact.byte_length, Buffer.byteLength(config.saved_texts[0], "utf8"));
  assert.ok(typeof artifact.created_at === "string" && Number.isFinite(Date.parse(artifact.created_at)));
  const reference = index => ({ workspace: baseline.task.reference.workspace,
    artifact_id: posts()[index].body.artifact_id, revision_id: posts()[index].body.revision_id,
    sha256: hash(config.saved_texts[index]) });
  assert.deepEqual(artifact.reference, reference(0));
  assert.notEqual(reference(0).artifact_id, reference(1).artifact_id);
  assert.notEqual(reference(0).revision_id, reference(1).revision_id);
  emit({ phase: "first_confirmed", reference: reference(0), pending: reference(1) }); await advance();

  const rejected = await incoming[1];
  // Establish the actual response before waiting for its intended UI outcome. A different
  // fixed status must fail here instead of being obscured by a later DOM wait timeout.
  stage = `partial-prefix-second-response-status-${[401, 403, 503].includes(rejected.status) ? rejected.status : "unexpected"}`;
  assert.equal(rejected.status, config.revoked ? 401 : 503);
  assert.deepEqual(rejected.body, { schema_version: 1, error: {
    code: config.revoked ? "invalid_session" : "service_unavailable", outcome: "unconfirmed" } });
  await state("task-edit-state", config.revoked ? "quarantined" : "unknown");
  assert.equal(posts().length, 2); assert.equal(puts().length, 0); assert.equal(reads(), readsBeforeSave);
  assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  assert.equal(await page.locator("#save-metadata-dialog").isVisible(), false);
  if (config.revoked) {
    stage = "partial-prefix-revoked-private-quarantine";
    assert.notEqual(await page.locator("#status").getAttribute("data-state"), "active");
    assert.deepEqual(await fields(), { title: "", filename: "", language: "", text: "" });
    for (const selector of ["#task-edit-item option", "#task-payload-select option", "#task-list button"])
      assert.equal(await page.locator(selector).count(), 0);
    for (const selector of ["#task-content-title", "#task-content-metadata", "#task-text", "#metadata-save-target", "#metadata-save-summary"])
      assert.equal(await page.locator(selector).textContent(), "");
    assert.equal(await page.locator("#task-metadata-editor").isVisible(), false);
    assert.equal(await page.locator("#task-save-progress").isVisible(), false);
    const body = await page.locator("body").textContent();
    assert.ok(![config.first_text, config.second_text, ...config.saved_texts, config.saved_title,
      ...config.saved_names].some(value => body.includes(value)), "quarantine cannot reveal confirmed or pending private buffers");
  } else {
    stage = "partial-prefix-timeout-retains-terminal-draft";
    assert.deepEqual(await fields(), draft);
    assert.equal(await page.locator("#task-save-progress").getAttribute("data-confirmed-artifacts"), "1");
    assert.equal(await page.locator("#task-save-progress").getAttribute("data-task-confirmed"), "false");
    for (const selector of ["#task-edit-title", "#task-edit-filename", "#task-edit-language", "#task-edit-text"])
      assert.equal(await page.locator(selector).getAttribute("readonly"), "");
    assert.equal(await page.locator("#task-text").textContent(), config.first_text);
  }
  await clean(); await flush(); emit({ phase: "terminal" }); await advance();
  stage = "terminal-translation-never-replays";
  for (const locale of ["ko", "zh-CN", "en"]) await page.locator("#locale").selectOption(locale);
  assert.equal(posts().length, 2); assert.equal(puts().length, 0); assert.equal(reads(), readsBeforeSave);
  assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  if (config.revoked) {
    assert.deepEqual(await fields(), { title: "", filename: "", language: "", text: "" });
    const body = await page.locator("body").textContent();
    assert.ok(![config.first_text, config.second_text, ...config.saved_texts, config.saved_title,
      ...config.saved_names].some(value => body.includes(value)), "translation cannot restore quarantined buffers");
  } else assert.deepEqual(await fields(), draft);
  assert.equal(calls.filter(call => call.method !== "GET").length, 3, "one natural login and two explicit Artifact POSTs only");
  for (const path of assets) assert.ok(calls.some(call => call.path === path), "all thirteen fixed assets required");
  await clean(); await flush(); await responses.close();
  await context.close(); context = undefined; emit({ phase: "complete" });
}

try { await run(await read()); }
catch (error) {
  const check = /^(?:request-contract|reply-(?:asset|publish|task))(?:-[a-z-]+)?$/.test(error?.check)
    ? error.check : error?.name === "TimeoutError" ? "browser-wait-timeout" : "fixture-assertion";
  emit({ phase: "failure", stage, check });
  process.stderr.write("platform partial-prefix browser fixture failed\n"); process.exitCode = 1;
}
finally {
  await context?.close().catch(() => {}); lines.close();
  if (directory) await fs.rm(directory, { recursive: true, force: true });
}
