import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { content, copy, hash } from "../platformTasks/data.mjs";

// Own loopback HTTP mock only: real production assets/CSP, no real credentials,
// authorization, SQL, TLS, persisted Artifact bytes or server COMMIT provenance.
const root = new URL("../../../engine/src/bin/cyanrex-platform/session_ui/", import.meta.url);
const files = ["app.js", "labels.js", "tasks.js", "task-contract.js", "task-edit.js", "request.js", "style.css",
  "body-contract.js", "task-save/taskContentSave.js", "task-save/taskSaveContract.js",
  "task-save/taskDraft.js", "task-save/taskPayloadLanguage.js"];
export const SESSION = "/platform/v1/session", LOGOUT = `${SESSION}/logout`;
export const INDEX = "/platform/v1/task-index", TASK = "/platform/v1/tasks/";
export const active = '{"schema_version":1,"authenticated":true}';
export const invalid = '{"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}';
export const conflict = '{"schema_version":1,"error":{"code":"stale_revision","outcome":"unconfirmed"}}';
export const failed = '{"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}}';
export async function waitFor(check) {
  const until = Date.now() + 4000;
  while (!check()) {
    assert.ok(Date.now() < until, "owned body fixture did not reach expected observation");
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}
export function published(record, original = content()) {
  const body = JSON.parse(record.body);
  return { schema_version: 1, artifact: { schema_version: 1,
    reference: { workspace: copy(original.task.reference.workspace), artifact_id: body.artifact_id,
      revision_id: body.revision_id, sha256: hash(body.text) },
    owner: copy(original.task.owner), sequence: 1, parent: null, kind: "cyanrex.task-text",
    title: original.task.title, media_type: "text/plain", byte_length: Buffer.byteLength(body.text, "utf8"),
    created_at: "2026-10-26T02:03:04Z" } };
}
export function saved(record, original = content()) {
  const manifest = copy(JSON.parse(record.body).manifest);
  return { schema_version: 1, task: { ...copy(original.task), title: manifest.title,
    input_refs: manifest.payload.map(item => copy(item.artifact)), revision: original.task.revision + 1,
    updated_at: "2026-10-26T02:03:04Z" }, manifest };
}

export async function setup(browser, options = {}) {
  let html = await readFile(new URL("index.html", root), "utf8");
  html = html.replace(/\sdata-task-text-publication=(?:"[^"]*"|'[^']*')/g, "");
  if (options.capability !== null && options.capability !== undefined)
    html = html.replace("<body", `<body data-task-text-publication="${options.capability}"`);
  const assets = new Map([["/platform/", { type: "text/html; charset=utf-8", body: html }]]);
  for (const file of files) {
    assets.set(`/platform/ui/${file}`, {
      type: `${file.endsWith("css") ? "text/css" : "text/javascript"}; charset=utf-8`,
      body: await readFile(new URL(file, root)),
    });
  }
  const rust = await readFile(new URL("../session_ui.rs", root), "utf8");
  const policy = rust.match(/const POLICY: &str = "([^"\n]+)";/)?.[1];
  assert.ok(policy, "use the actual production CSP");
  const headers = { "content-security-policy": policy, "cache-control": "no-store", pragma: "no-cache",
    "x-content-type-options": "nosniff", "referrer-policy": "no-referrer", "x-frame-options": "DENY",
    "cross-origin-resource-policy": "same-origin", "cross-origin-opener-policy": "same-origin" };
  const records = [], violations = [], errors = [], pending = new Set();
  let f;
  const server = createServer(async (request, response) => {
    pending.add(response); response.on("close", () => pending.delete(response));
    const url = new URL(request.url, f.origin), record = { method: request.method, path: url.pathname,
      url: request.url, headers: request.headers, body: "", response };
    const chunks = []; let size = 0;
    for await (const chunk of request) {
      size += chunk.byteLength;
      if (size > 2 * 1024 * 1024) { response.destroy(); return; }
      chunks.push(chunk);
    }
    record.body = Buffer.concat(chunks).toString("utf8");
    records.push(record);
    const asset = assets.get(record.path);
    if (record.method === "GET" && asset && !url.search) {
      response.writeHead(200, { ...headers, "content-type": asset.type }); response.end(asset.body); return;
    }
    if (record.path.startsWith("/platform/v1/")) {
      if (options.api) return options.api(record, f);
      return f.respond(record, record.path === SESSION ? 200 : 503, record.path === SESSION ? active : failed);
    }
    response.writeHead(404, headers); response.end();
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  let context;
  try {
    context = await browser.newContext({ viewport: options.viewport || { width: 1180, height: 1050 },
      locale: "en-US", hasTouch: Boolean(options.touch), serviceWorkers: "block" });
    if (options.fastTimeout) await context.addInitScript(() => {
      const original = window.setTimeout;
      window.setTimeout = function(callback, delay, ...args) {
        return original.call(this, callback, delay === 10_000 ? 250 : delay, ...args);
      };
    });
    if (options.init) await context.addInitScript(options.init);
    await context.route("**/*", route => {
      if (new URL(route.request().url()).origin !== origin) {
        violations.push("blocked unexpected external traffic"); return route.abort();
      }
      return route.continue();
    });
    const page = await context.newPage(); page.setDefaultTimeout(4000);
    page.on("pageerror", () => errors.push("unexpected page error"));
    f = { page, context, origin, records,
      api: path => records.filter(record => record.path.startsWith("/platform/v1/") && (!path || path === record.path)),
      puts: () => records.filter(record => record.method === "PUT"),
      posts: () => records.filter(record => record.method === "POST" && record.path.endsWith("/text-artifacts")),
      writes: () => records.filter(record => record.method !== "GET" && record.path.startsWith("/platform/v1/")),
      holdHeaders(record, status, extra = {}) {
        assert.ok(!record.response.headersSent && !record.response.writableEnded);
        record.response.writeHead(status, { ...headers, "content-type": "application/json", ...extra });
        record.response.flushHeaders();
      },
      respond(record, status, body, extra = {}) {
        if (record.response.destroyed || record.response.writableEnded) return false;
        record.response.writeHead(status, { ...headers, "content-type": "application/json", ...extra });
        record.response.end(typeof body === "string" || Buffer.isBuffer(body) ? body : JSON.stringify(body)); return true;
      },
      async session(expected = "active") { await page.waitForFunction(value => document.querySelector("#status")?.dataset.state === value, expected); },
      async state(expected) { await page.waitForFunction(value => document.querySelector("#task-edit-state")?.dataset.state === value, expected); },
      async readState(expected) { await page.waitForFunction(value => document.querySelector("#task-read-state")?.dataset.state === value, expected); },
      async ready() { await page.waitForFunction(() => document.body.dataset.busy === "false"); },
      async safe() {
        assert.deepEqual(violations, []); assert.deepEqual(errors, []);
        assert.ok(f.api().every(record => record.headers.authorization === undefined));
        assert.ok(f.api().every(record => record.method === "GET" || record.path === LOGOUT
          || (record.method === "PUT" && /^\/platform\/v1\/tasks\/[a-f0-9-]{36}\/content$/.test(record.path))
          || (options.capability === "enabled" && record.method === "POST"
            && /^\/platform\/v1\/tasks\/[a-f0-9-]{36}\/text-artifacts$/.test(record.path))),
        "only fixed Task body publication and content replacement writes are permitted");
        assert.ok(records.every(record => !record.path.startsWith("/api/") && !record.path.startsWith("/engine/")));
        assert.deepEqual(await page.evaluate(() => [localStorage.length, sessionStorage.length]), [0, 0]);
      },
      async close() {
        await context.close(); for (const response of pending) response.destroy();
        server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
        assert.equal(server.listening, false);
      },
    };
    return f;
  } catch (error) {
    await context?.close(); for (const response of pending) response.destroy();
    server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); throw error;
  }
}
