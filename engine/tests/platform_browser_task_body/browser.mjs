// Real native host, SQL and immutable files. No fulfilled requests or injected platform Cookie.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import * as fs from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
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
const execute = promisify(execFile);
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;
const policy = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src data:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'";
const normalize = value => value.replace(/\r\n?/g, "\n");
const hash = value => createHash("sha256").update(value, "utf8").digest("hex");
let context, directory, stage = "browser-startup";

async function prepare(config) {
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"]) {
    assert.ok(process.env[key] && isAbsolute(process.env[key]), "explicit browser dependency required");
  }
  assert.equal(typeof config.tls, "boolean", "explicit transport selector required");
  assert.equal(new URL(config.origin).protocol, config.tls ? "https:" : "http:");
  if (config.tls) assert.ok(!existsSync(join(homedir(), ".pki/nssdb")),
    "legacy user NSS overrides private XDG trust; TLS is blocked, never bypassed");
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  directory = await fs.mkdtemp(join(tmpdir(), "cyanrex-platform-browser-body-"));
  await fs.chmod(directory, 0o700);
  const data = join(directory, "data");
  await fs.mkdir(data, { recursive: true, mode: 0o700 });
  const launch = () => chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    viewport: { width: 1100, height: 1000 },
    env: { ...process.env, XDG_DATA_HOME: data,
      XDG_CONFIG_HOME: join(directory, "config"), XDG_CACHE_HOME: join(directory, "cache") },
  });
  if (config.tls) {
    assert.ok(typeof config.ca_file === "string" && isAbsolute(config.ca_file));
    const nss = join(data, "pki/nssdb");
    await fs.mkdir(nss, { recursive: true, mode: 0o700 });
    await execute("certutil", ["-N", "--empty-password", "-d", `sql:${nss}`], { timeout: 10000 });
    context = await launch();
    let rejected = false;
    try { await context.pages()[0].goto(`${config.origin}/platform/`, { timeout: 10000 }); }
    catch (error) { rejected = String(error.message).includes("ERR_CERT_AUTHORITY_INVALID"); }
    assert.ok(rejected, "empty private CA store must reject the certificate, not time out");
    await context.close(); context = undefined;
    emit({ phase: "ca_rejected" }); await advance();
    // Generated private XDG trust only. Never change HOME, user NSS or system trust.
    await execute("certutil", ["-A", "-d", `sql:${nss}`, "-n", "Cyanrex disposable body fixture CA",
      "-t", "C,,", "-i", config.ca_file], { timeout: 10000 });
  }
  context = await launch();
  return context.pages()[0];
}

async function run(config) {
  const page = await prepare(config), origin = new URL(config.origin).origin;
  const taskPath = `/platform/v1/tasks/${config.text_id}`;
  const publishPath = `${taskPath}/text-artifacts`, savePath = `${taskPath}/content`;
  const staticPaths = new Set(["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js",
    "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js",
    "/platform/ui/task-edit.js", "/platform/ui/request.js", "/platform/ui/body-contract.js",
    "/platform/ui/task-save/taskContentSave.js", "/platform/ui/task-save/taskSaveContract.js",
    "/platform/ui/task-save/taskDraft.js", "/platform/ui/task-save/taskPayloadLanguage.js"]);
  const calls = [], observations = createObservations(), errors = [];
  const responses = await createCdpResponses(await context.newCDPSession(page));
  let privateToken = null;
  page.on("pageerror", () => errors.push("unexpected body browser runtime error"));
  page.on("request", request => {
    const url = new URL(request.url()), method = request.method();
    const record = { path: url.pathname, query: url.search, method };
    calls.push(record);
    observations.track("request-contract", async () => {
      assert.equal(url.origin, origin); assert.ok(!url.username && !url.password && !url.hash);
      const headers = await request.allHeaders();
      assert.equal(headers.authorization, undefined);
      if (staticPaths.has(url.pathname) || url.pathname === "/favicon.ico") {
        assert.equal(method, "GET"); assert.equal(url.search, "");
        assert.equal(headers.cookie, undefined); return;
      }
      assert.equal(headers["x-cyanrex-platform-request"], "1");
      assert.equal(headers["sec-fetch-mode"], "same-origin");
      assert.equal(headers["sec-fetch-site"], "same-origin");
      if (url.pathname === "/platform/v1/session/login") {
        assert.equal(method, "POST"); assert.equal(headers.origin, origin); assert.equal(url.search, ""); return;
      }
      if (url.pathname === publishPath || url.pathname === savePath) {
        assert.equal(method, url.pathname === publishPath ? "POST" : "PUT");
        assert.equal(url.search, ""); assert.equal(headers.origin, origin);
        assert.equal(headers.cookie, `cyanrex_platform_session=${privateToken}`);
        assert.match(headers["content-type"], /^application\/json(?:;\s*charset=utf-8)?$/i);
        record.body = JSON.parse(request.postData()); return;
      }
      assert.equal(method, "GET", "no unbound publication, create/status write, logout or retry");
      if (url.pathname === "/platform/v1/session") { assert.equal(url.search, ""); return; }
      if (url.pathname === "/platform/v1/task-index") {
        assert.deepEqual([...url.searchParams], [["limit", "20"]]); return;
      }
      assert.equal(url.pathname, taskPath); assert.equal(url.search, "");
    });
  });
  page.on("response", response => {
    const path = new URL(response.url()).pathname;
    if (!staticPaths.has(path)) return;
    observations.track("reply-asset", async () => {
      const headers = await response.allHeaders();
      assert.equal(response.status(), 200); assert.equal(headers["cache-control"], "no-store");
      assert.equal(headers["content-security-policy"], policy);
      assert.equal(headers["x-content-type-options"], "nosniff");
      assert.equal(headers["referrer-policy"], "no-referrer");
      assert.equal(headers["cross-origin-resource-policy"], "same-origin");
      assert.equal(headers["cross-origin-opener-policy"], "same-origin");
      assert.equal(headers["x-frame-options"], "DENY");
      assert.equal(headers["set-cookie"], undefined);
    });
  });
  const expectReplies = (path, method, count = 1) => {
    // Every identical POST is synchronously queued before confirmation. Never arm after a body read.
    const incoming = responses.expectMany(`${origin}${path}`, method, count), replies = [];
    incoming.forEach((promise, index) => observations.track(method === "POST" ? "reply-publish"
      : method === "PUT" ? "reply-save" : "reply-task", async () => { replies[index] = await promise; }));
    return replies;
  };
  const flush = async () => {
    observations.track("reply-publish", () => responses.check());
    await observations.drain(); assert.deepEqual(errors, []);
  };
  const count = (path, method = "GET") => calls.filter(call => call.path === path && call.method === method).length;
  const posts = () => calls.filter(call => call.path === publishPath && call.method === "POST");
  const puts = () => calls.filter(call => call.path === savePath && call.method === "PUT");
  const reads = () => calls.filter(call => call.method === "GET" && call.path.startsWith("/platform/v1/")).length;
  const state = async (id, expected) => {
    stage = `${id}-${expected}`;
    try {
      await page.waitForFunction(({ id, expected }) => document.getElementById(id)?.dataset.state === expected
        && document.body.dataset.busy === "false", { id, expected }, { timeout: 10000 });
    } catch (error) { await flush(); throw error; }
    await flush();
  };
  const text = () => page.locator("#task-text").textContent();
  const fields = async () => ({ title: await page.locator("#task-edit-title").inputValue(),
    filename: await page.locator("#task-edit-filename").inputValue(),
    language: await page.locator("#task-edit-language").inputValue(),
    text: await page.locator("#task-edit-text").inputValue() });
  const progress = async (artifacts, task = false) => {
    assert.equal(await page.locator("#task-save-progress").getAttribute("data-confirmed-artifacts"), String(artifacts));
    assert.equal(await page.locator("#task-save-progress").getAttribute("data-task-confirmed"), String(task));
  };
  const clean = async () => {
    const stored = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    const privateValues = [privateToken, config.password, config.otp, config.first_text, config.second_text,
      ...config.saved_texts, config.conflict_text, config.revoked_text, config.conflict_title, config.revoked_title];
    assert.ok(!privateValues.filter(Boolean).some(value => stored.includes(value)), "private state must not persist in Web Storage");
    const body = await page.locator("body").textContent();
    assert.ok(!body.includes(privateToken) && !body.includes(config.foreign_title));
    assert.ok(!(await page.evaluate(() => document.cookie)).includes("cyanrex_platform_session"));
    assert.equal(page.frames().length, 1); assert.equal(context.pages().length, 1); await flush();
  };
  const open = async expected => {
    await page.locator("#load-tasks").click(); await state("task-read-state", "listed");
    const reply = expectReplies(taskPath, "GET");
    await page.locator(`#task-list button[data-task-id="${config.text_id}"]`).click();
    await state("task-read-state", "opened");
    assert.equal(await text(), expected[0]);
    assert.equal(await page.locator("#task-payload-select option").count(), 2);
    assert.equal(await page.evaluate(() => Object.hasOwn(globalThis, "__privateTaskExecuted")), false);
    assert.equal(await page.locator("#task-text script, #task-text img").count(), 0);
    assert.equal(reply[0].status, 200); return reply[0].body;
  };
  const edit = async (title, names, languages, texts) => {
    if (await page.locator("#edit-task").isVisible()) {
      await page.locator("#edit-task").click(); await state("task-edit-state", "clean");
    } else assert.equal(await page.locator("#task-edit-state").getAttribute("data-state"), "saved");
    assert.equal(await page.locator("#task-edit-item option").count(), 2);
    assert.equal(await page.locator("#task-edit-text").isVisible(), true);
    await page.locator("#task-edit-title").fill(title);
    for (let index = 0; index < 2; index++) {
      await page.locator("#task-edit-item").selectOption({ index });
      await page.locator("#task-edit-filename").fill(names[index]);
      await page.locator("#task-edit-language").fill(languages[index]);
      await page.locator("#task-edit-text").fill(normalize(texts[index]));
    }
    await page.locator("#task-edit-item").selectOption({ index: 0 }); await state("task-edit-state", "dirty");
  };
  const review = async revision => {
    await page.locator("#save-task-metadata").click();
    await page.locator("#save-metadata-dialog").waitFor({ state: "visible" });
    await page.waitForFunction(() => document.activeElement?.id === "cancel-save-metadata", null, { timeout: 10000 });
    const target = await page.locator("#metadata-save-target").textContent();
    assert.ok(target.includes(config.text_id)); assert.match(target, new RegExp(`\\b${revision}$`));
  };
  const assertPublication = (record, reply, baseline, expectedText) => {
    assert.deepEqual(Object.keys(record.body).sort(), ["artifact_id", "expected", "revision_id", "schema_version", "text"]);
    assert.equal(record.body.schema_version, 1); assert.equal(record.body.text, expectedText);
    assert.deepEqual(record.body.expected, { task: baseline.task, manifest: baseline.manifest });
    for (const key of ["artifact_id", "revision_id"]) {
      assert.match(record.body[key], uuid);
      assert.ok(!baseline.task.input_refs.some(pin => pin[key] === record.body[key]));
    }
    assert.equal(reply.status, 201);
    assert.deepEqual(Object.keys(reply.body).sort(), ["artifact", "schema_version"]);
    assert.equal(reply.body.schema_version, 1);
    const artifact = reply.body.artifact;
    assert.deepEqual(Object.keys(artifact).sort(), ["byte_length", "created_at", "kind", "media_type", "owner", "parent", "reference", "schema_version", "sequence", "title"]);
    assert.equal(artifact.schema_version, 1); assert.equal(artifact.sequence, 1); assert.equal(artifact.parent, null);
    assert.equal(artifact.kind, "cyanrex.task-text"); assert.equal(artifact.media_type, "text/plain");
    assert.equal(artifact.title, baseline.task.title); assert.deepEqual(artifact.owner, baseline.task.owner);
    assert.equal(artifact.byte_length, Buffer.byteLength(expectedText, "utf8"));
    assert.ok(typeof artifact.created_at === "string" && Number.isFinite(Date.parse(artifact.created_at)));
    assert.deepEqual(artifact.reference, { workspace: baseline.task.reference.workspace,
      artifact_id: record.body.artifact_id, revision_id: record.body.revision_id, sha256: hash(expectedText) });
    return artifact.reference;
  };
  const assertDeniedPublication = (record, reply, baseline, expectedText, status, code) => {
    assert.deepEqual(Object.keys(record.body).sort(), ["artifact_id", "expected", "revision_id", "schema_version", "text"]);
    assert.equal(record.body.schema_version, 1); assert.equal(record.body.text, expectedText);
    assert.deepEqual(record.body.expected, { task: baseline.task, manifest: baseline.manifest });
    assert.match(record.body.artifact_id, uuid); assert.match(record.body.revision_id, uuid);
    assert.equal(reply.status, status);
    assert.deepEqual(reply.body, { schema_version: 1, error: { code, outcome: "unconfirmed" } });
  };

  await page.goto(`${origin}/platform/`); await state("status", "unauthenticated");
  assert.equal(await page.locator("body").getAttribute("data-task-text-publication"), "enabled");
  assert.equal(count("/platform/v1/task-index"), 0);
  assert.ok(!Object.hasOwn(config, "otp")); emit({ phase: "ready_login" });
  const fresh = await read(); assert.deepEqual(Object.keys(fresh), ["otp"]); assert.match(fresh.otp, /^[0-9]{6}$/);
  config.otp = fresh.otp;
  await page.locator("#username").fill(config.username); await page.locator("#password").fill(config.password);
  await page.locator("#otp").fill(config.otp); await page.locator("#login").click(); await state("status", "active");
  assert.equal(count("/platform/v1/session/login", "POST"), 1); assert.equal(count("/platform/v1/session"), 2);
  const cookies = (await context.cookies(`${origin}/platform/v1/session`)).filter(value => value.name === "cyanrex_platform_session");
  assert.equal(cookies.length, 1); const cookie = cookies[0]; privateToken = cookie.value;
  assert.match(privateToken, uuid); assert.equal(cookie.domain, new URL(origin).hostname);
  assert.equal(cookie.path, "/platform/v1"); assert.equal(cookie.httpOnly, true); assert.equal(cookie.sameSite, "Strict");
  assert.equal(cookie.secure, config.tls); assert.equal(cookie.expires, -1);
  assert.equal(await page.locator("#password").inputValue(), ""); assert.equal(await page.locator("#otp").inputValue(), "");
  await clean(); emit({ phase: "authenticated", digest: hash(privateToken) }); await advance();

  const initial = await open([config.first_text, config.second_text]);
  assert.equal(initial.task.revision, 1); assert.equal(initial.task.title, config.text_title);
  await page.locator("#edit-task").click(); await state("task-edit-state", "clean");
  assert.equal(await page.locator("#task-edit-text").inputValue(), normalize(config.first_text));
  await page.locator("#task-edit-item").selectOption({ index: 1 });
  assert.equal(await page.locator("#task-edit-text").inputValue(), normalize(config.second_text));
  await page.locator("#task-edit-item").selectOption({ index: 0 }); await state("task-edit-state", "clean");
  assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  // The first editor is already open, and remains the same fixed-slot draft.
  await page.locator("#task-edit-title").fill(config.saved_title);
  for (let index = 0; index < 2; index++) {
    await page.locator("#task-edit-item").selectOption({ index });
    await page.locator("#task-edit-filename").fill(config.saved_names[index]);
    await page.locator("#task-edit-language").fill(config.saved_languages[index]);
    await page.locator("#task-edit-text").fill(normalize(config.saved_texts[index]));
  }
  await page.locator("#task-edit-item").selectOption({ index: 0 }); await state("task-edit-state", "dirty");
  const draft = await fields(), readsBeforeSave = reads();
  await review(1); assert.equal(posts().length, 0); assert.equal(puts().length, 0);
  await page.locator("#cancel-save-metadata").click(); await page.locator("#save-metadata-dialog").waitFor({ state: "hidden" });
  assert.deepEqual(await fields(), draft); assert.equal(posts().length, 0); assert.equal(puts().length, 0);
  await review(1);
  const published = expectReplies(publishPath, "POST", 2), replaced = expectReplies(savePath, "PUT");
  await page.locator("#confirm-save-metadata").click(); await state("task-edit-state", "saved");
  stage = "save-exact-write-count";
  assert.equal(posts().length, 2); assert.equal(puts().length, 1);
  const references = published.map((reply, index) => {
    stage = index === 0 ? "first-publication-exact-confirmation" : "second-publication-exact-confirmation";
    return assertPublication(posts()[index], reply, initial, config.saved_texts[index]);
  });
  stage = "publication-targets-distinct";
  assert.notEqual(references[0].artifact_id, references[1].artifact_id);
  assert.notEqual(references[0].revision_id, references[1].revision_id);
  const savedManifest = structuredClone(initial.manifest); savedManifest.title = config.saved_title;
  savedManifest.payload = references.map((artifact, index) => ({ kind: "text", artifact,
    filename: config.saved_names[index], language: config.saved_languages[index] }));
  stage = "save-request-exact-body";
  assert.deepEqual(puts()[0].body, { schema_version: 1, expected_revision: 1, manifest: savedManifest });
  stage = "save-response-exact-confirmation";
  assert.equal(replaced[0].status, 200);
  assert.deepEqual(Object.keys(replaced[0].body).sort(), ["manifest", "schema_version", "task"]);
  const saved = replaced[0].body; assert.equal(saved.schema_version, 1); assert.deepEqual(saved.manifest, savedManifest);
  assert.deepEqual(saved.task, { ...initial.task, revision: 2, title: config.saved_title,
    input_refs: references, updated_at: saved.task.updated_at });
  assert.ok(typeof saved.task.updated_at === "string" && Number.isFinite(Date.parse(saved.task.updated_at)));
  stage = "save-exact-order-and-progress";
  assert.deepEqual(calls.filter(call => [publishPath, savePath].includes(call.path)).map(call => call.method), ["POST", "POST", "PUT"]);
  await progress(2, true); assert.equal(reads(), readsBeforeSave, "confirmed save cannot perform a hidden reload");
  stage = "saved-rendered-local-text";
  assert.equal(await page.locator("#task-content-title").textContent(), config.saved_title);
  for (let index = 0; index < 2; index++) {
    await page.locator("#task-payload-select").selectOption({ index }); assert.equal(await text(), config.saved_texts[index]);
  }
  assert.equal(await page.evaluate(() => Object.hasOwn(globalThis, "__privateTaskExecuted")), false);
  stage = "save-fixed-production-assets";
  for (const path of staticPaths) assert.ok(count(path) >= 1, "all thirteen fixed production assets must be requested");
  await clean(); emit({ phase: "saved" }); await advance();

  const conflictNames = [config.conflict_name, config.saved_names[1]];
  const conflictLanguages = [config.conflict_language, config.saved_languages[1]];
  await edit(config.conflict_title, conflictNames, conflictLanguages, [config.conflict_text, config.saved_texts[1]]);
  const conflictDraft = await fields(), readsBeforeConflict = reads();
  await review(2); emit({ phase: "conflict_ready" }); await advance();
  const conflict = expectReplies(publishPath, "POST");
  await page.locator("#confirm-save-metadata").click(); await state("task-edit-state", "conflict");
  stage = "conflict-exact-count-and-baseline";
  assert.equal(posts().length, 3); assert.equal(puts().length, 1);
  assertDeniedPublication(posts()[2], conflict[0], saved, config.conflict_text, 409, "stale_revision");
  stage = "conflict-retained-draft-no-retry";
  await progress(0); assert.equal(reads(), readsBeforeConflict, "conflict cannot reload or rebase");
  assert.deepEqual(await fields(), conflictDraft); assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  await clean(); emit({ phase: "conflict" }); await advance();

  // Discard is explicit and Cancel-focused. No failed attempt can adopt the newer head itself.
  await page.locator("#discard-task-edit").click(); await page.locator("#task-leave-dialog").waitFor({ state: "visible" });
  assert.equal(await page.evaluate(() => document.activeElement?.id), "cancel-task-leave");
  await page.locator("#cancel-task-leave").click(); await page.locator("#task-leave-dialog").waitFor({ state: "hidden" });
  assert.deepEqual(await fields(), conflictDraft); assert.equal(reads(), readsBeforeConflict);
  await page.locator("#discard-task-edit").click(); await page.locator("#confirm-task-leave").click();
  await page.locator("#task-leave-dialog").waitFor({ state: "hidden" });
  assert.equal(reads(), readsBeforeConflict); assert.equal(posts().length, 3); assert.equal(puts().length, 1);
  const reopened = await open(config.saved_texts);
  assert.equal(reopened.task.revision, 3); assert.equal(reopened.task.title, config.admin_title);
  const adminManifest = structuredClone(savedManifest); adminManifest.title = config.admin_title;
  assert.deepEqual(reopened.manifest, adminManifest);
  await edit(config.revoked_title, [config.revoked_name, config.saved_names[1]],
    [config.revoked_language, config.saved_languages[1]], [config.revoked_text, config.saved_texts[1]]);
  const readsBeforeDenial = reads(); await review(3); emit({ phase: "revoked_ready" }); await advance();
  const denied = expectReplies(publishPath, "POST");
  await page.locator("#confirm-save-metadata").click(); await state("task-edit-state", "quarantined");
  stage = "denial-exact-count-and-baseline";
  assert.equal(posts().length, 4); assert.equal(puts().length, 1);
  assertDeniedPublication(posts()[3], denied[0], reopened, config.revoked_text, 401, "invalid_session");
  stage = "denial-private-dom-quarantine";
  assert.equal(reads(), readsBeforeDenial, "denial cannot retry or probe");
  assert.notEqual(await page.locator("#status").getAttribute("data-state"), "active");
  assert.deepEqual(await fields(), { title: "", filename: "", language: "", text: "" });
  for (const selector of ["#task-edit-item option", "#task-payload-select option", "#task-list button"])
    assert.equal(await page.locator(selector).count(), 0);
  for (const selector of ["#task-content-title", "#task-content-metadata", "#task-text", "#metadata-save-target", "#metadata-save-summary"])
    assert.equal(await page.locator(selector).textContent(), "");
  assert.equal(await page.locator("#task-metadata-editor").isVisible(), false);
  assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  const body = await page.locator("body").textContent();
  assert.ok(![config.first_text, config.second_text, ...config.saved_texts, config.conflict_text,
    config.revoked_text, config.saved_title, config.admin_title, config.revoked_title, config.revoked_name]
    .some(value => body.includes(value)), "quarantine must remove private read and input DOM");
  assert.equal(calls.filter(call => call.method !== "GET").length, 6, "one login, four explicit POSTs and one PUT only");
  assert.equal(count("/platform/v1/task-index"), 2); assert.equal(count(taskPath), 2);
  await clean(); emit({ phase: "denied" }); await advance();
  await responses.close(); await context.close(); context = undefined; emit({ phase: "complete" });
}

try { await run(await read()); }
catch (error) {
  const check = /^(?:request-contract|reply-(?:asset|publish|save|task))(?:-[a-z-]+)?$/.test(error?.check)
    ? error.check : error?.name === "TimeoutError" ? "browser-wait-timeout" : "fixture-assertion";
  emit({ phase: "failure", stage, check });
  process.stderr.write("platform body browser fixture failed\n"); process.exitCode = 1;
}
finally {
  await context?.close().catch(() => {}); lines.close();
  if (directory) await fs.rm(directory, { recursive: true, force: true });
}
