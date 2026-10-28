// Real Chrome + synthetic loopback HTTP only; no credentials, database, routing or extra fetch.
import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { mkdtemp, chmod, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { pathToFileURL } from "node:url";
import { createCdpResponses } from "../../engine/tests/platform_browser_task_metadata/cdp-responses.mjs";

let directory, context, page, server, responses, origin;
const requests = [], unexpected = [];
const unicode = "合作教学 한국어 😀 café";
let repeated = 0;
const headers = type => ({ "Content-Type": type, "Cache-Control": "no-store" });
function serve(request, response) {
  const path = new URL(request.url, "http://127.0.0.1").pathname;
  requests.push({ path, method: request.method });
  if (path === "/" && request.method === "GET") {
    response.writeHead(200, headers("text/html; charset=utf-8"));
    response.end('<!doctype html><title>Synthetic CDP fixture</title><link rel="icon" href="data:,">');
    return;
  }
  if (path === "/favicon.ico" && request.method === "GET") { response.writeHead(204); response.end(); return; }
  if (path === "/tiny" && request.method === "GET") {
    response.writeHead(201, headers("application/json")); response.end('{"tiny":true}'); return;
  }
  if (path === "/chunks" && request.method === "GET") {
    const bytes = Buffer.from(JSON.stringify({ text: unicode }));
    const split = bytes.indexOf(Buffer.from("😀")) + 2;
    response.writeHead(200, headers("application/json; charset=utf-8"));
    response.write(bytes.subarray(0, split));
    const timer = setTimeout(() => response.end(bytes.subarray(split)), 40);
    response.once("close", () => clearTimeout(timer)); return;
  }
  if (path === "/binary-json" && request.method === "GET") {
    // Let Chrome choose its natural CDP base64 transport; never fabricate protocol events.
    response.writeHead(200, headers("application/octet-stream"));
    response.end(Buffer.from(JSON.stringify({ binaryMime: unicode }))); return;
  }
  if (["/empty", "/invalid"].includes(path) && request.method === "GET") {
    response.writeHead(200, headers("application/json"));
    response.end(path === "/empty" ? "" : "synthetic non-JSON"); return;
  }
  if (path === "/abort" && request.method === "GET") {
    response.writeHead(200, headers("application/json")); response.flushHeaders();
    response.write('{"unfinished":');
    const timer = setTimeout(() => response.end("true}"), 5_000);
    response.once("close", () => clearTimeout(timer)); return;
  }
  if (path === "/repeat" && request.method === "PUT") {
    response.writeHead(200, headers("application/json"));
    response.end(JSON.stringify({ sequence: ++repeated })); return;
  }
  unexpected.push({ path, method: request.method }); response.writeHead(404); response.end();
}

before(async () => {
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"]) {
    assert.ok(process.env[key] && isAbsolute(process.env[key]), `set explicit absolute ${key}`);
    assert.ok((await stat(process.env[key])).isFile(), `${key} must name an existing file`);
  }
  directory = await mkdtemp(join(tmpdir(), "cyanrex-metadata-cdp-browser-"));
  await chmod(directory, 0o700);
  server = createServer(serve); server.listen(0, "127.0.0.1"); await once(server, "listening");
  origin = `http://127.0.0.1:${server.address().port}`;
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  context = await chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    env: { ...process.env, XDG_DATA_HOME: join(directory, "data"),
      XDG_CONFIG_HOME: join(directory, "config"), XDG_CACHE_HOME: join(directory, "cache") },
  });
  page = context.pages()[0];
  responses = await createCdpResponses(await context.newCDPSession(page));
  await page.goto(`${origin}/`, { waitUntil: "load" });
});

after(async () => {
  try { await responses?.close(); } finally {
    try { await context?.close(); } finally {
      if (server) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
      if (directory) await rm(directory, { recursive: true, force: true });
    }
  }
});

async function fetchObserved(path, { method = "GET", abort = false } = {}) {
  const observed = responses.expect(`${origin}${path}`, method);
  const ui = page.evaluate(async ({ path, method, abort }) => {
    const controller = new AbortController();
    const response = await fetch(path, { method, cache: "no-store", signal: controller.signal });
    const body = response.text();
    if (abort) {
      controller.abort();
      return { aborted: await body.then(() => false, () => true) };
    }
    return { status: response.status, text: await body };
  }, { path, method, abort });
  return Promise.allSettled([observed, ui]);
}
function count(path) { return requests.filter(request => request.path === path); }
function success(results, expected) {
  const [observed, ui] = results;
  assert.equal(observed.status, "fulfilled"); assert.deepEqual(observed.value, expected);
  assert.equal(ui.status, "fulfilled"); assert.equal(ui.value.status, expected.status);
  assert.deepEqual(JSON.parse(ui.value.text), expected.body);
}
function hardFailure(results) {
  assert.equal(results[0].status, "rejected"); assert.ok(results[0].reason instanceof Error);
  assert.equal(results[1].status, "fulfilled");
}

test("CDP observer captures a tiny fast JSON reply without a second GET", { timeout: 15_000 }, async () => {
  success(await fetchObserved("/tiny"), { status: 201, body: { tiny: true } });
  assert.deepEqual(count("/tiny"), [{ path: "/tiny", method: "GET" }]);
});

test("CDP observer joins delayed chunks across a UTF-8 code point", { timeout: 15_000 }, async () => {
  success(await fetchObserved("/chunks"), { status: 200, body: { text: unicode } });
  assert.deepEqual(count("/chunks"), [{ path: "/chunks", method: "GET" }]);
});

test("CDP observer decodes Chrome's natural binary-MIME JSON transport", { timeout: 15_000 }, async () => {
  success(await fetchObserved("/binary-json"), { status: 200, body: { binaryMime: unicode } });
  assert.deepEqual(count("/binary-json"), [{ path: "/binary-json", method: "GET" }]);
});

test("empty and malformed JSON are hard failures rather than invented acknowledgements", { timeout: 15_000 }, async () => {
  for (const path of ["/empty", "/invalid"]) {
    hardFailure(await fetchObserved(path));
    assert.deepEqual(count(path), [{ path, method: "GET" }]);
  }
});

test("an aborted original UI fetch cannot produce an observed successful reply", { timeout: 15_000 }, async () => {
  const results = await fetchObserved("/abort", { abort: true });
  hardFailure(results); assert.deepEqual(results[1].value, { aborted: true });
  assert.deepEqual(count("/abort"), [{ path: "/abort", method: "GET" }]);
});

test("consecutive same-URL PUTs retain their distinct original replies and exact request counts", { timeout: 15_000 }, async () => {
  for (const sequence of [1, 2]) {
    success(await fetchObserved("/repeat", { method: "PUT" }), { status: 200, body: { sequence } });
  }
  assert.deepEqual(count("/repeat"), [{ path: "/repeat", method: "PUT" }, { path: "/repeat", method: "PUT" }]);
  assert.deepEqual(unexpected, []);
  assert.equal(requests.filter(request => !["/", "/favicon.ico"].includes(request.path)).length, 8);
  assert.equal(count("/").length, 1);
});
