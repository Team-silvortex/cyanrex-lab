// Real native host + real Source/Task/Artifact storage; no fulfilled requests or injected Cookie.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import { join, isAbsolute } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { createInterface } from "node:readline";
import { createObservations } from "./observations.mjs";

const lines = createInterface({ input: process.stdin, terminal: false });
const input = lines[Symbol.asyncIterator]();
const read = async () => {
  const next = await input.next();
  assert.ok(!next.done && Buffer.byteLength(next.value) < 16 * 1024, "bounded private IPC required");
  return JSON.parse(next.value);
};
const emit = value => process.stdout.write(`${JSON.stringify(value)}\n`);
const advance = async () => assert.deepEqual(await read(), { continue: true });
let context, directory, stage = "browser-startup";

async function run(config) {
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"]) {
    assert.ok(process.env[key] && isAbsolute(process.env[key]), "explicit browser dependency required");
  }
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  directory = await fs.mkdtemp(join(tmpdir(), "cyanrex-platform-browser-metadata-"));
  await fs.chmod(directory, 0o700);
  context = await chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    viewport: { width: 1100, height: 1000 },
    env: { ...process.env, XDG_DATA_HOME: join(directory, "data"),
      XDG_CONFIG_HOME: join(directory, "config"), XDG_CACHE_HOME: join(directory, "cache") },
  });
  const page = context.pages()[0], origin = new URL(config.origin).origin;
  const taskPath = `/platform/v1/tasks/${config.text_id}`, savePath = `${taskPath}/content`;
  const staticPaths = new Set(["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js",
    "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js",
    "/platform/ui/task-edit.js", "/favicon.ico"]);
  const calls = [], replies = [], observations = createObservations(), errors = [];
  page.on("pageerror", () => errors.push("unexpected metadata browser runtime error"));
  page.on("request", request => {
    const url = new URL(request.url()), method = request.method();
    const record = { path: url.pathname, query: url.search, method };
    calls.push(record);
    observations.track("request-contract", async () => {
      assert.equal(url.origin, origin); assert.ok(!url.username && !url.password && !url.hash);
      if (staticPaths.has(url.pathname)) {
        assert.equal(method, "GET"); assert.equal(url.search, ""); return;
      }
      const headers = await request.allHeaders();
      assert.equal(headers["x-cyanrex-platform-request"], "1");
      if (url.pathname === "/platform/v1/session/login") {
        assert.equal(method, "POST"); assert.equal(headers.origin, origin); assert.equal(url.search, ""); return;
      }
      if (url.pathname === savePath) {
        assert.equal(method, "PUT"); assert.equal(url.search, ""); assert.equal(headers.origin, origin);
        assert.match(headers["content-type"], /^application\/json(?:;\s*charset=utf-8)?$/i);
        record.body = JSON.parse(request.postData());
        assert.deepEqual(Object.keys(record.body).sort(), ["expected_revision", "manifest", "schema_version"]);
        assert.equal(record.body.schema_version, 1); return;
      }
      assert.equal(method, "GET", "no publication, Task create/status write, logout or implicit retry");
      if (url.pathname === "/platform/v1/session") { assert.equal(url.search, ""); return; }
      if (url.pathname === "/platform/v1/task-index") {
        assert.deepEqual([...url.searchParams], [["limit", "20"]]); return;
      }
      assert.equal(url.pathname, taskPath); assert.equal(url.search, "");
    });
  });
  const expectReply = (path, method) => {
    // Register before the explicit action. UI state and Node's response/body callbacks
    // travel independently; a rendered saved state is not a response-observer barrier.
    const incoming = page.waitForResponse(response => response.request().method() === method
      && response.url() === `${origin}${path}`, { timeout: 10000 });
    observations.track(method === "PUT" ? "reply-save" : "reply-task", async () => {
      const response = await incoming;
      replies.push({ path, status: response.status(), body: await response.json() });
    });
  };
  const flush = async () => { await observations.drain(); assert.deepEqual(errors, []); };
  const count = (path, method = "GET") => calls.filter(call => call.path === path && call.method === method).length;
  const puts = () => calls.filter(call => call.path === savePath && call.method === "PUT");
  const getCount = () => calls.filter(call => call.method === "GET" && call.path.startsWith("/platform/v1/")).length;
  const state = async (id, expected) => {
    stage = `${id}-${expected}`;
    await page.waitForFunction(({ id, expected }) => document.getElementById(id)?.dataset.state === expected
      && document.body.dataset.busy === "false", { id, expected }, { timeout: 10000 });
    await flush();
  };
  const text = () => page.locator("#task-text").evaluate(element => "value" in element ? element.value : element.textContent);
  const fields = async () => ({ title: await page.locator("#task-edit-title").inputValue(),
    filename: await page.locator("#task-edit-filename").inputValue(),
    language: await page.locator("#task-edit-language").inputValue() });
  const clean = async token => {
    const stored = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    assert.ok(![token, config.password, config.otp, config.first_text, config.second_text,
      config.conflict_title, config.revoked_title].some(value => stored.includes(value)), "private state must not persist in Web Storage");
    assert.ok(!(await page.locator("body").innerText()).includes(config.foreign_title));
    assert.ok(!(await page.evaluate(() => document.cookie)).includes("cyanrex_platform_session"));
    assert.equal(page.frames().length, 1); assert.equal(context.pages().length, 1); await flush();
  };
  const open = async () => {
    await page.locator("#load-tasks").click(); await state("task-read-state", "listed");
    expectReply(taskPath, "GET");
    await page.locator(`#task-list button[data-task-id="${config.text_id}"]`).click();
    await state("task-read-state", "opened");
    assert.equal(await text(), config.first_text);
    assert.equal(await page.locator("#task-payload-select option").count(), 2);
    assert.equal(await page.evaluate(() => Object.hasOwn(globalThis, "__privateTaskExecuted")), false);
    assert.equal(await page.locator("#task-text script, #task-text img").count(), 0);
  };
  const edit = async (title, filename, language) => {
    if (await page.locator("#edit-task").isVisible()) {
      await page.locator("#edit-task").click(); await state("task-edit-state", "clean");
    } else {
      // A confirmed save can keep the now-clean editor open for the next distinct draft.
      assert.equal(await page.locator("#task-edit-state").getAttribute("data-state"), "saved");
    }
    assert.equal(await page.locator("#task-edit-item option").count(), 2);
    await page.locator("#task-edit-title").fill(title);
    await page.locator("#task-edit-item").selectOption({ index: 0 });
    await page.locator("#task-edit-filename").fill(filename);
    await page.locator("#task-edit-language").fill(language);
    await state("task-edit-state", "dirty");
  };
  const confirmDialog = async revision => {
    await page.locator("#save-task-metadata").click();
    await page.locator("#save-metadata-dialog").waitFor({ state: "visible" });
    assert.equal(await page.evaluate(() => document.activeElement?.id), "cancel-save-metadata");
    const target = await page.locator("#metadata-save-target").textContent();
    assert.ok(target.includes(config.text_id)); assert.match(target, new RegExp(`\\b${revision}$`));
  };
  const expectedManifest = (base, title, filename, language) => {
    const manifest = structuredClone(base); manifest.title = title;
    manifest.payload[0].filename = filename; manifest.payload[0].language = language;
    return manifest;
  };
  const assertPut = async (index, revision, manifest, status, code) => {
    stage = "put-observation-drain"; await flush();
    stage = "put-request-count"; assert.equal(puts().length, index + 1);
    stage = "put-request-exact-body";
    assert.deepEqual(puts()[index].body, { schema_version: 1, expected_revision: revision, manifest });
    const reply = replies.filter(reply => reply.path === savePath)[index];
    stage = "put-response-observed"; assert.ok(reply);
    stage = "put-response-status";
    assert.equal(reply.status, status);
    stage = "put-response-exact-body";
    if (code) assert.deepEqual(reply.body, { schema_version: 1, error: { code, outcome: "unconfirmed" } });
    else {
      assert.deepEqual(Object.keys(reply.body).sort(), ["manifest", "schema_version", "task"]);
      assert.equal(reply.body.schema_version, 1); assert.equal(reply.body.task.revision, revision + 1);
      assert.equal(reply.body.task.title, manifest.title); assert.deepEqual(reply.body.manifest, manifest);
    }
  };

  await page.goto(`${origin}/platform/`); await state("status", "unauthenticated");
  assert.equal(count("/platform/v1/task-index"), 0);
  assert.ok(!Object.hasOwn(config, "otp")); emit({ phase: "ready_login" });
  const fresh = await read(); assert.deepEqual(Object.keys(fresh), ["otp"]); assert.match(fresh.otp, /^[0-9]{6}$/); config.otp = fresh.otp;
  await page.locator("#username").fill(config.username); await page.locator("#password").fill(config.password);
  await page.locator("#otp").fill(config.otp); await page.locator("#login").click(); await state("status", "active");
  assert.equal(count("/platform/v1/session/login", "POST"), 1); assert.equal(count("/platform/v1/session"), 2);
  assert.equal(count("/platform/v1/task-index"), 0);
  const cookies = (await context.cookies(`${origin}/platform/v1/session`)).filter(c => c.name === "cyanrex_platform_session");
  assert.equal(cookies.length, 1); const cookie = cookies[0];
  assert.match(cookie.value, /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/);
  assert.equal(cookie.path, "/platform/v1"); assert.equal(cookie.httpOnly, true);
  assert.equal(cookie.sameSite, "Strict"); assert.equal(cookie.secure, false);
  const digest = createHash("sha256").update(cookie.value).digest("hex");
  await clean(cookie.value); emit({ phase: "authenticated", digest }); await advance();

  await open();
  const initial = replies.find(reply => reply.path === taskPath).body;
  assert.equal(initial.task.revision, 1); assert.equal(initial.task.title, config.text_title);
  assert.equal(initial.manifest.payload[1].language, "future-notes-v9");
  await edit(config.saved_title, config.saved_names[0], config.saved_languages[0]);
  await page.locator("#task-edit-item").selectOption({ index: 1 });
  assert.equal(await page.locator("#task-edit-language").inputValue(), config.saved_languages[1]);
  await page.locator("#task-edit-filename").fill(config.saved_names[1]);
  await page.locator("#task-edit-item").selectOption({ index: 0 });
  const savedManifest = expectedManifest(initial.manifest, config.saved_title, config.saved_names[0], config.saved_languages[0]);
  savedManifest.payload[1].filename = config.saved_names[1];
  const draft = await fields(), readsBeforeSave = getCount();
  await confirmDialog(1); assert.equal(puts().length, 0);
  await page.locator("#cancel-save-metadata").click();
  await page.locator("#save-metadata-dialog").waitFor({ state: "hidden" });
  assert.deepEqual(await fields(), draft); assert.equal(puts().length, 0);
  await confirmDialog(1); expectReply(savePath, "PUT"); await page.locator("#confirm-save-metadata").click();
  await state("task-edit-state", "saved"); await assertPut(0, 1, savedManifest, 200);
  stage = "saved-no-hidden-read";
  assert.equal(getCount(), readsBeforeSave, "confirmed save must not perform a hidden reload");
  stage = "saved-rendered-title";
  assert.equal(await page.locator("#task-content-title").textContent(), config.saved_title);
  stage = "saved-first-content";
  assert.equal(await text(), config.first_text);
  stage = "saved-second-content";
  await page.locator("#task-payload-select").selectOption({ index: 1 }); assert.equal(await text(), config.second_text);
  stage = "saved-return-first-content";
  await page.locator("#task-payload-select").selectOption({ index: 0 });
  stage = "saved-private-boundaries";
  await clean(cookie.value); emit({ phase: "saved" }); await advance();

  await edit(config.conflict_title, config.conflict_name, config.conflict_language);
  const conflictDraft = await fields(), readsBeforeConflict = getCount();
  const conflictManifest = expectedManifest(savedManifest, config.conflict_title, config.conflict_name, config.conflict_language);
  await confirmDialog(2); emit({ phase: "conflict_ready" }); await advance();
  expectReply(savePath, "PUT");
  await page.locator("#confirm-save-metadata").click(); await state("task-edit-state", "conflict");
  await assertPut(1, 2, conflictManifest, 409, "stale_revision");
  assert.equal(getCount(), readsBeforeConflict, "conflict must not reload or rebase the draft");
  assert.deepEqual(await fields(), conflictDraft); assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  await clean(cookie.value); emit({ phase: "conflict" }); await advance();

  // The user explicitly cancels a discard once, then confirms discarding held work. Only a
  // subsequent explicit list/reopen may fetch the independently committed revision 3.
  await page.locator("#discard-task-edit").click(); await page.locator("#task-leave-dialog").waitFor({ state: "visible" });
  assert.equal(await page.evaluate(() => document.activeElement?.id), "cancel-task-leave");
  await page.locator("#cancel-task-leave").click();
  await page.locator("#task-leave-dialog").waitFor({ state: "hidden" });
  assert.deepEqual(await fields(), conflictDraft); assert.equal(getCount(), readsBeforeConflict);
  await page.locator("#discard-task-edit").click(); await page.locator("#task-leave-dialog").waitFor({ state: "visible" });
  await page.locator("#confirm-task-leave").click(); await page.locator("#task-leave-dialog").waitFor({ state: "hidden" });
  assert.equal(getCount(), readsBeforeConflict); assert.equal(puts().length, 2);
  await open();
  const reopened = replies.filter(reply => reply.path === taskPath).at(-1).body;
  assert.equal(reopened.task.revision, 3); assert.equal(reopened.task.title, config.admin_title);
  const adminManifest = structuredClone(savedManifest); adminManifest.title = config.admin_title;
  assert.deepEqual(reopened.manifest, adminManifest);
  await edit(config.revoked_title, config.revoked_name, config.revoked_language);
  const revokedManifest = expectedManifest(adminManifest, config.revoked_title, config.revoked_name, config.revoked_language);
  const readsBeforeDenial = getCount();
  await confirmDialog(3); emit({ phase: "revoked_ready" }); await advance();
  expectReply(savePath, "PUT");
  await page.locator("#confirm-save-metadata").click(); await state("task-edit-state", "quarantined");
  await assertPut(2, 3, revokedManifest, 401, "invalid_session");
  assert.equal(getCount(), readsBeforeDenial, "authorization denial must not retry or probe automatically");
  assert.notEqual(await page.locator("#status").getAttribute("data-state"), "active");
  assert.deepEqual(await fields(), { title: "", filename: "", language: "" });
  assert.equal(await page.locator("#task-edit-item option").count(), 0);
  assert.equal(await page.locator("#task-list button").count(), 0);
  assert.equal(await page.locator("#task-content-title").textContent(), "");
  assert.equal(await text(), ""); assert.equal(await page.locator("#task-payload-select option").count(), 0);
  assert.equal(await page.locator("#save-task-metadata").isDisabled(), true);
  const body = await page.locator("body").innerText();
  assert.ok(![config.first_text, config.second_text, config.saved_title, config.admin_title,
    config.revoked_title, config.revoked_name].some(value => body.includes(value)), "quarantine must remove private read and input DOM");
  assert.equal(calls.filter(call => call.method !== "GET").length, 4, "one login and exactly three separately confirmed PUT attempts");
  assert.equal(count("/platform/v1/task-index"), 2); assert.equal(count(taskPath), 2);
  await clean(cookie.value); emit({ phase: "denied" }); await advance();
  await context.close(); context = undefined; emit({ phase: "complete" });
}

try { await run(await read()); }
catch (error) {
  const check = /^(?:request|reply)-[a-z-]+$/.test(error?.check) ? error.check
    : error?.name === "TimeoutError" ? "browser-wait-timeout" : "fixture-assertion";
  emit({ phase: "failure", stage, check });
  process.stderr.write("platform metadata browser fixture failed\n"); process.exitCode = 1;
}
finally { await context?.close().catch(() => {}); lines.close(); if (directory) await fs.rm(directory, { recursive: true, force: true }); }
