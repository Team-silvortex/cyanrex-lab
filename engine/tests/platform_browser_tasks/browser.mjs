// Real native host and real Source/Task/Artifact storage. No network fulfillment or injected Cookie.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as fs from "node:fs/promises";
import { join, isAbsolute } from "node:path";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import { createInterface } from "node:readline";

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
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"]) assert.ok(
    process.env[key] && isAbsolute(process.env[key]), "explicit browser dependency required");
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  directory = await fs.mkdtemp(join(tmpdir(), "cyanrex-platform-browser-tasks-"));
  await fs.chmod(directory, 0o700);
  context = await chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    viewport: { width: 1100, height: 900 },
    env: { ...process.env, XDG_DATA_HOME: join(directory, "data"),
      XDG_CONFIG_HOME: join(directory, "config"), XDG_CACHE_HOME: join(directory, "cache") },
  });
  const page = context.pages()[0], origin = new URL(config.origin).origin;
  const staticPaths = new Set(["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js",
    "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js", "/platform/ui/task-edit.js", "/favicon.ico"]);
  const calls = [], checks = [], errors = [];
  page.on("pageerror", () => errors.push("unexpected Task browser runtime error"));
  page.on("request", request => {
    const url = new URL(request.url()), method = request.method();
    calls.push({ path: url.pathname, query: url.search, method });
    checks.push((async () => {
      assert.equal(url.origin, origin); assert.ok(!url.username && !url.password && !url.hash);
      if (staticPaths.has(url.pathname)) { assert.equal(method, "GET"); assert.equal(url.search, ""); return; }
      const headers = await request.allHeaders();
      assert.equal(headers["x-cyanrex-platform-request"], "1");
      if (url.pathname === "/platform/v1/session/login") {
        assert.equal(method, "POST"); assert.equal(headers.origin, origin); assert.equal(url.search, ""); return;
      }
      assert.equal(method, "GET", "Task browsing must never dispatch business writes or logout");
      if (url.pathname === "/platform/v1/session") { assert.equal(url.search, ""); return; }
      if (url.pathname === "/platform/v1/task-index") {
        const query = [...url.searchParams];
        assert.ok(query.length === 1 || query.length === 2);
        assert.equal(url.searchParams.get("limit"), "20");
        assert.ok(query.every(([key]) => ["limit", "after"].includes(key)));
        if (query.length === 2) assert.equal(url.searchParams.get("after"), config.own[19]);
        return;
      }
      const id = url.pathname.match(/^\/platform\/v1\/tasks\/([0-9a-f-]{36})$/)?.[1];
      assert.ok(id && config.own.includes(id), "only server-listed own exact Task IDs may be reopened");
      assert.equal(url.search, "");
    })());
  });
  const flush = async () => { await Promise.all(checks); assert.deepEqual(errors, []); };
  const count = path => calls.filter(call => call.path === path).length;
  const sessionState = async value => {
    stage = `session-${value}`;
    await page.waitForFunction(expected => document.querySelector("#status")?.dataset.state === expected
      && document.body.dataset.busy === "false", value, { timeout: 10000 }); await flush();
  };
  const taskState = async value => {
    stage = `task-${value}`;
    await page.waitForFunction(expected => document.querySelector("#task-read-state")?.dataset.state === expected,
      value, { timeout: 10000 }); await flush();
  };
  const text = () => page.locator("#task-text").evaluate(element => "value" in element ? element.value : element.textContent);
  const ids = () => page.locator("#task-list button[data-task-id]").evaluateAll(buttons => buttons.map(button => button.dataset.taskId));
  const clean = async token => {
    const stored = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    assert.ok(![token, config.password, config.otp, config.first_text, config.second_text].some(secret => stored.includes(secret)), "private data must not persist in Web Storage");
    assert.ok(!(await page.locator("body").innerText()).includes(config.foreign_title));
    assert.ok(!(await page.evaluate(() => document.cookie)).includes("cyanrex_platform_session"));
    assert.equal(page.frames().length, 1); assert.equal(context.pages().length, 1); await flush();
  };
  await page.goto(`${origin}/platform/`); await sessionState("unauthenticated");
  assert.equal(count("/platform/v1/task-index"), 0);
  assert.ok(!Object.hasOwn(config, "otp")); emit({ phase: "ready_login" });
  const fresh = await read(); assert.deepEqual(Object.keys(fresh), ["otp"]); assert.match(fresh.otp, /^[0-9]{6}$/); config.otp = fresh.otp;
  await page.locator("#username").fill(config.username); await page.locator("#password").fill(config.password);
  await page.locator("#otp").fill(config.otp); await page.locator("#login").click(); await sessionState("active");
  assert.equal(count("/platform/v1/session/login"), 1); assert.equal(count("/platform/v1/session"), 2);
  assert.equal(count("/platform/v1/task-index"), 0, "Task loading is explicit, never a login side effect");
  const cookies = (await context.cookies(`${origin}/platform/v1/session`)).filter(c => c.name === "cyanrex_platform_session");
  assert.equal(cookies.length, 1); const cookie = cookies[0];
  assert.ok(/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(cookie.value));
  assert.equal(cookie.path, "/platform/v1"); assert.equal(cookie.httpOnly, true); assert.equal(cookie.sameSite, "Strict"); assert.equal(cookie.secure, false);
  const digest = createHash("sha256").update(cookie.value).digest("hex");
  await clean(cookie.value); emit({ phase: "authenticated", digest }); await advance();
  await page.locator("#load-tasks").click(); await taskState("listed");
  assert.deepEqual(await ids(), config.own.slice(0, 20)); assert.equal(await page.locator("#next-tasks").isEnabled(), true);
  await page.locator("#next-tasks").click(); await taskState("listed");
  assert.deepEqual(await ids(), config.own.slice(20)); assert.equal(await page.locator("#next-tasks").isEnabled(), false);
  assert.equal(count("/platform/v1/task-index"), 2);
  await page.locator("#load-tasks").click(); await taskState("listed"); assert.deepEqual(await ids(), config.own.slice(0, 20));
  const open = async (id, expected) => { await page.locator(`#task-list button[data-task-id="${id}"]`).click(); await taskState(expected); };
  await open(config.text_id, "opened"); assert.equal(await page.locator("#task-content-title").textContent(), config.text_title);
  assert.equal(await page.locator("#task-payload-select option").count(), 2); assert.equal(await text(), config.first_text);
  assert.equal(await page.locator("#task-text script, #task-text img").count(), 0);
  assert.equal(await page.evaluate(() => Object.hasOwn(globalThis, "__privateTaskExecuted")), false);
  await page.locator("#task-payload-select").selectOption({ index: 1 }); assert.equal(await text(), config.second_text);
  await open(config.empty_id, "empty"); assert.equal(await page.locator("#task-content-title").textContent(), config.empty_title);
  assert.equal(await page.locator("#task-payload-select option").count(), 0); assert.equal(await text(), "");
  await open(config.text_id, "opened"); assert.equal(await text(), config.first_text);
  await clean(cookie.value); emit({ phase: "read_complete" }); await advance();
  await page.locator("#load-tasks").click(); await taskState("unknown");
  assert.deepEqual(await ids(), []); assert.equal(await page.locator("#task-content-title").textContent(), "");
  assert.equal(await page.locator("#task-payload-select option").count(), 0); assert.equal(await text(), "");
  assert.notEqual(await page.locator("#status").getAttribute("data-state"), "active");
  const body = await page.locator("body").innerText();
  assert.ok(![config.text_title, config.first_text, config.second_text].some(value => body.includes(value)), "permission denial must discard old private display content");
  const retained = (await context.cookies(`${origin}/platform/v1/session`)).filter(c => c.name === "cyanrex_platform_session");
  assert.equal(retained.length, 1); assert.ok(retained[0].value === cookie.value, "membership denial is not an implicit logout");
  assert.equal(calls.filter(call => call.method !== "GET").length, 1, "the sole mutation is the explicitly submitted login");
  assert.equal(count("/platform/v1/task-index"), 4); await clean(cookie.value);
  emit({ phase: "denied" }); await advance();
  await context.close(); context = undefined; emit({ phase: "complete" });
}

try { await run(await read()); }
catch { emit({ phase: "failure", stage }); process.stderr.write("platform Task browser fixture failed\n"); process.exitCode = 1; }
finally { await context?.close().catch(() => {}); lines.close(); if (directory) await fs.rm(directory, { recursive: true, force: true }); }
