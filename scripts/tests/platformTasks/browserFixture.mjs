import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";

// Owned HTTP/static fixture only. Session and Task APIs are MOCKED; no PostgreSQL, TLS,
// teaching Engine, real authentication or persisted-content provenance is established here.
const root = new URL("../../../engine/src/bin/cyanrex-platform/session_ui/", import.meta.url);
const assets = new Map([
  ["/platform/", ["index.html", "text/html; charset=utf-8"]],
  ["/platform/ui/app.js", ["app.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/labels.js", ["labels.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/style.css", ["style.css", "text/css; charset=utf-8"]],
  ["/platform/ui/tasks.js", ["tasks.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/task-contract.js", ["task-contract.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/task-edit.js", ["task-edit.js", "text/javascript; charset=utf-8"]],
]);
export const SESSION = "/platform/v1/session";
export const LOGOUT = `${SESSION}/logout`;
export const INDEX = "/platform/v1/task-index";
export const TASK = "/platform/v1/tasks/";
export const active = '{"schema_version":1,"authenticated":true}';
export const invalid = '{"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}';
export const unavailable = '{"schema_version":1,"error":{"code":"not_found","outcome":"unconfirmed"}}';
export const failed = '{"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}}';

export async function waitFor(check) {
  const until = Date.now() + 4000;
  while (!check()) {
    assert.ok(Date.now() < until, "owned browser fixture did not reach the expected observation");
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}

export async function setup(browser, options = {}) {
  const bytes = new Map();
  for (const [path, [file, type]] of assets) {
    try { bytes.set(path, { type, body: await readFile(new URL(file, root)) }); }
    catch (error) {
      // Test-first: absent new assets must produce a real UI failure, not a fixture exception.
      if (error.code !== "ENOENT" || !["tasks.js", "task-contract.js"].includes(file)) throw error;
    }
  }
  const rust = await readFile(new URL("../session_ui.rs", root), "utf8");
  const policy = rust.match(/const POLICY: &str = "([^"\n]+)";/)?.[1];
  assert.ok(policy, "use the actual production CSP, not a relaxed fixture policy");
  const headers = { "content-security-policy": policy, "cache-control": "no-store",
    pragma: "no-cache", "x-content-type-options": "nosniff", "referrer-policy": "no-referrer",
    "cross-origin-resource-policy": "same-origin", "cross-origin-opener-policy": "same-origin",
    "x-frame-options": "DENY" };
  const records = [], violations = [], errors = [], pending = new Set();
  let f;
  const server = createServer(async (request, response) => {
    pending.add(response); response.on("close", () => pending.delete(response));
    const url = new URL(request.url, f.origin);
    const record = { method: request.method, path: url.pathname, url: request.url,
      headers: request.headers, body: "", response };
    for await (const chunk of request) {
      record.body += chunk.toString("utf8");
      if (Buffer.byteLength(record.body) > 64 * 1024) { response.destroy(); return; }
    }
    records.push(record);
    const asset = bytes.get(url.pathname);
    if (asset && record.method === "GET" && !url.search) {
      response.writeHead(200, { ...headers, "content-type": asset.type }); response.end(asset.body); return;
    }
    if (record.path.startsWith("/platform/v1/")) {
      if (options.api) return options.api(record, f);
      return f.respond(record, record.path === SESSION ? 200 : 404,
        record.path === SESSION ? active : unavailable);
    }
    response.writeHead(404, headers); response.end();
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  let context;
  try {
    context = await browser.newContext({ viewport: options.viewport || { width: 1100, height: 950 },
      locale: "en-US", hasTouch: Boolean(options.touch) });
    if (options.init) await context.addInitScript(options.init);
    await context.route("**/*", route => {
      if (new URL(route.request().url()).origin !== origin) {
        violations.push("blocked unexpected external traffic"); return route.abort();
      }
      return route.continue();
    });
    const page = await context.newPage(); page.setDefaultTimeout(4000);
    page.on("pageerror", error => errors.push(error.message));
    f = { page, context, origin, records, violations, errors,
      api: (path) => records.filter(record => record.path.startsWith("/platform/v1/") && (!path || record.path === path)),
      tasks: () => records.filter(record => record.path === INDEX || record.path.startsWith(TASK)),
      respond(record, status, body, extra = {}) {
        if (record.response.destroyed || record.response.writableEnded) return false;
        record.response.writeHead(status, { ...headers, "content-type": "application/json", ...extra });
        record.response.end(typeof body === "string" || Buffer.isBuffer(body) ? body : JSON.stringify(body)); return true;
      },
      async state(value) { await page.waitForFunction(expected => document.querySelector("#task-read-state")?.dataset.state === expected, value); },
      async session(value = "active") { await page.waitForFunction(expected => document.querySelector("#status")?.dataset.state === expected, value); },
      async ready() { await page.waitForFunction(() => document.body.dataset.busy === "false"); },
      async clear() {
        assert.equal(await page.locator("#task-list button[data-task-id]").count(), 0);
        assert.equal(await page.locator("#task-content-title").textContent(), "");
        assert.equal(await page.locator("#task-text").textContent(), "");
        assert.equal(await page.locator("#task-payload-select option").count(), 0);
      },
      async safe() {
        assert.deepEqual(violations, []); assert.deepEqual(errors, []);
        assert.ok(f.tasks().every(record => record.method === "GET" && record.body === ""), "Task UI is strictly read-only");
        assert.ok(records.every(record => !record.path.startsWith("/api/") && !record.path.startsWith("/engine/")));
        const stored = await page.evaluate(() => [localStorage.length, sessionStorage.length]);
        assert.deepEqual(stored, [0, 0]);
      },
      async close() {
        await context.close();
        for (const response of pending) response.destroy();
        server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
        assert.equal(server.listening, false, "only the fixture-owned server is stopped");
      },
    };
    return f;
  } catch (error) {
    await context?.close();
    for (const response of pending) response.destroy();
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); throw error;
  }
}
