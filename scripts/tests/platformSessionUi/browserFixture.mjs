import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";

// This is an owned HTTP/static harness with MOCK Session APIs. It proves UI behavior only,
// never real credentials, PostgreSQL authorization, TLS trust or commit provenance.
const assetRoot = new URL("../../../engine/src/bin/cyanrex-platform/session_ui/", import.meta.url);
const routes = new Map([
  ["/platform/", ["index.html", "text/html; charset=utf-8"]],
  ["/platform/ui/app.js", ["app.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/labels.js", ["labels.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/style.css", ["style.css", "text/css; charset=utf-8"]],
  ["/platform/ui/tasks.js", ["tasks.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/task-contract.js", ["task-contract.js", "text/javascript; charset=utf-8"]],
  ["/platform/ui/task-edit.js", ["task-edit.js", "text/javascript; charset=utf-8"]],
]);
export const SESSION = "/platform/v1/session";
export const LOGIN = `${SESSION}/login`;
export const LOGOUT = `${SESSION}/logout`;
export const authenticated = '{"schema_version":1,"authenticated":true}';
export const signedOut = '{"schema_version":1,"ok":true}';
export const invalidSession = '{"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}';
export const password = "Synthetic_Private_Password_SENTINEL_1d4b";
export const otp = "953817";
export const username = "teacher_fixture_only";

export async function waitFor(check, message = "owned browser fixture did not reach the expected state") {
  const until = Date.now() + 4000;
  while (!check()) {
    assert.ok(Date.now() < until, message);
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}

export async function setup(browser, options = {}) {
  const assets = new Map();
  for (const [path, [file, type]] of routes) assets.set(path, {
    type, bytes: await readFile(new URL(file, assetRoot)),
  });
  const rust = await readFile(new URL("../session_ui.rs", assetRoot), "utf8");
  const policy = rust.match(/const POLICY: &str = "([^"\n]+)";/)?.[1];
  assert.ok(policy, "read the actual fixed Rust CSP as data, never execute Rust source");
  const headers = { "cache-control": "no-store", pragma: "no-cache", "x-content-type-options": "nosniff",
    "content-security-policy": policy, "referrer-policy": "no-referrer",
    "cross-origin-resource-policy": "same-origin", "cross-origin-opener-policy": "same-origin",
    "x-frame-options": "DENY" };
  const records = [], violations = [], logs = [], pageErrors = [], pending = new Set();
  let f;
  const server = createServer(async (request, response) => {
    pending.add(response); response.on("close", () => pending.delete(response));
    const url = new URL(request.url, f.origin);
    const record = { method: request.method, path: url.pathname, url: request.url,
      headers: request.headers, body: "", response, aborted: false };
    request.on("aborted", () => { record.aborted = true; });
    for await (const bytes of request) {
      record.body += bytes.toString("utf8");
      if (Buffer.byteLength(record.body) > 64 * 1024) { response.destroy(); return; }
    }
    records.push(record);
    const asset = assets.get(url.pathname);
    if (asset && request.method === "GET" && !url.search) {
      response.writeHead(200, { ...headers, "content-type": asset.type }); response.end(asset.bytes); return;
    }
    if ([SESSION, LOGIN, LOGOUT].includes(url.pathname)) {
      if (options.api) return options.api(record, f);
      return f.respond(record, request.method === "GET" ? 401 : 200,
        request.method === "GET" ? invalidSession : url.pathname === LOGOUT ? signedOut : authenticated);
    }
    response.writeHead(404, { ...headers, "content-type": "application/json" });
    response.end('{"schema_version":1,"error":{"code":"not_found","outcome":"not_attempted"}}');
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  let context;
  try {
  context = await browser.newContext({ viewport: options.viewport || { width: 1100, height: 900 },
    locale: "en-US", hasTouch: Boolean(options.touch) });
  if (options.fastTimeout) await context.addInitScript(() => {
    const original = window.setTimeout;
    window.setTimeout = function(callback, delay, ...args) {
      return original.call(this, callback, delay === 10_000 ? 250 : delay, ...args);
    };
  });
  if (options.init) await context.addInitScript(options.init);
  await context.route("**/*", route => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { violations.push("Blocked unexpected external request"); return route.abort(); }
    if (options.blockScript && url.pathname === "/platform/ui/app.js") return route.abort();
    return route.continue();
  });
  const page = await context.newPage(); page.setDefaultTimeout(4000);
  page.on("console", message => logs.push(message.text()));
  page.on("pageerror", error => pageErrors.push(error.message));
  f = { page, context, origin, records, logs, pageErrors, violations,
    apiRecords: (path, method) => records.filter(record => [SESSION, LOGIN, LOGOUT].includes(record.path)
      && (!path || record.path === path) && (!method || record.method === method)),
    respond(record, status, body, extra = {}) {
      if (record.response.destroyed || record.response.writableEnded) return false;
      record.response.writeHead(status, { ...headers, "content-type": "application/json", ...extra });
      record.response.end(body); return true;
    },
    async state(expected) { await page.waitForFunction(value => document.querySelector("#status")?.dataset.state === value, expected); },
    async ready() { await page.locator("#login:enabled").waitFor(); },
    async fill() {
      await page.locator("#username").fill(username); await page.locator("#password").fill(password);
      await page.locator("#otp").fill(otp);
    },
    async noLeaks() {
      assert.equal(new URL(page.url()).origin, origin); assert.equal(new URL(page.url()).search, "");
      const surface = await page.evaluate(() => ({ url: location.href, text: document.body.textContent,
        local: JSON.stringify(localStorage), session: JSON.stringify(sessionStorage),
        password: document.querySelector("#password").value, otp: document.querySelector("#otp").value }));
      for (const secret of [password, otp]) {
        assert.ok(!JSON.stringify(surface).includes(secret), "private sentinel escaped into visible/storage state");
        assert.ok(!logs.some(message => message.includes(secret)), "private sentinel escaped into console output");
        assert.ok(!records.some(record => record.url.includes(secret)), "private sentinel escaped into a URL");
      }
      assert.deepEqual(violations, []); assert.deepEqual(pageErrors, []);
    },
    async close() {
      await context.close();
      for (const response of pending) response.destroy();
      server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
    },
  };
  return f;
  } catch (error) {
    await context?.close();
    for (const response of pending) response.destroy();
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
    throw error;
  }
}
