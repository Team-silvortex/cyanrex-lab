// Real Chrome + owned synthetic HTTP regression. Loads the production mounted request.js;
// no database, private data, fulfilled route, fetch replacement, retry or TLS bypass.
import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { createServer } from "node:http";
import { once } from "node:events";
import { mkdtemp, chmod, rm, stat, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { pathToFileURL } from "node:url";
import { createCdpResponses } from "../../engine/tests/platform_browser_task_metadata/cdp-responses.mjs";
import { loadingFailurePart } from "../../engine/tests/platform_browser_task_metadata/observations.mjs";

const task = "/platform/v1/tasks/00000000-0000-0000-0000-000000000010";
const marker = "public synthetic 合作教学 한국어 😀 café";
const cases = [
  { name: "native fetch.text length-close", mode: "text", framing: "length-close" },
  { name: "mounted request length-close", mode: "mounted", framing: "length-close" },
  { name: "mounted request retained parent signal length-close", mode: "mounted-retained", framing: "length-close" },
  { name: "mounted request chunked split UTF8", mode: "mounted", framing: "chunked" },
  { name: "chunked control native fetch.text", mode: "text", framing: "chunked" },
  { name: "mounted request 1MiB length-close", mode: "mounted", framing: "length-close", large: true },
  { name: "mounted conflict PUT length-close", mode: "mounted", framing: "length-close", method: "PUT", status: 409 },
  { name: "mounted publication POST chunked", mode: "mounted", framing: "chunked", method: "POST", status: 201 },
];
const fixtureModule = `
import { platformRequest } from '/platform/ui/request.js';
globalThis.exercise = async ({ mode, path, method, body }) => {
  const controller = new AbortController();
  if (mode === 'mounted-retained') globalThis.retained = controller;
  if (mode.startsWith('mounted')) return platformRequest(path, method,
    { ...(body === undefined ? {} : { body }), signal: controller.signal });
  const response = await fetch(path, { method, mode: 'same-origin', credentials: 'same-origin',
    cache: 'no-store', redirect: 'error', signal: controller.signal,
    headers: { 'X-Cyanrex-Platform-Request': '1' } });
  if (mode !== 'text') throw new Error('unsupported synthetic mode');
  return { status: response.status, raw: await response.text() };
};
globalThis.fixtureReady = true;
`;
const policy = "default-src 'none'; script-src 'self'; connect-src 'self'; img-src data:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'";
const baseHeaders = type => ({ "content-type": type, "cache-control": "no-store",
  "content-security-policy": policy, "x-content-type-options": "nosniff" });
let directory, context, server, origin, requestSource, current;
const violations = [], requests = [];

async function serve(request, response) {
  const path = new URL(request.url, "http://127.0.0.1").pathname;
  if (path === "/" && request.method === "GET") {
    response.writeHead(200, baseHeaders("text/html; charset=utf-8"));
    response.end('<!doctype html><title>Mounted transport CDP fixture</title><link rel="icon" href="data:,"><script type="module" src="/fixture.mjs"></script>'); return;
  }
  if (path === "/fixture.mjs" && request.method === "GET") {
    response.writeHead(200, baseHeaders("text/javascript; charset=utf-8")); response.end(fixtureModule); return;
  }
  if (path === "/platform/ui/request.js" && request.method === "GET") {
    response.writeHead(200, baseHeaders("text/javascript; charset=utf-8")); response.end(requestSource); return;
  }
  if (path === "/favicon.ico" && request.method === "GET") { response.writeHead(204); response.end(); return; }
  const selected = current;
  if (!selected || path !== selected.path || request.method !== selected.method || request.url !== path) {
    violations.push("unexpected request"); response.writeHead(404); response.end(); return;
  }
  const parts = []; let size = 0;
  for await (const part of request) { size += part.byteLength; if (size > 32) throw new Error("bounded synthetic request required"); parts.push(part); }
  requests.push({ path, method: request.method, body: Buffer.concat(parts).toString("utf8") });
  if (request.headers.authorization || request.headers.cookie) violations.push("unexpected credential");
  if (request.headers["x-cyanrex-platform-request"] !== "1") violations.push("missing request marker");
  const headers = { ...baseHeaders("application/json; charset=utf-8") };
  if (selected.framing === "length-close") {
    headers["content-length"] = String(selected.bytes.length); headers.connection = "close";
    response.writeHead(selected.status, headers); response.end(selected.bytes); return;
  }
  response.writeHead(selected.status, headers);
  const split = selected.bytes.indexOf(Buffer.from("😀")) + 2;
  assert.ok(split > 1 && split < selected.bytes.length);
  response.write(selected.bytes.subarray(0, split));
  const timer = setTimeout(() => response.end(selected.bytes.subarray(split)), 40);
  response.once("close", () => clearTimeout(timer));
}

before(async () => {
  for (const key of ["CYANREX_PLAYWRIGHT_MODULE", "CYANREX_CHROMIUM_PATH"]) {
    assert.ok(process.env[key] && isAbsolute(process.env[key]), `set explicit absolute ${key}`);
    assert.ok((await stat(process.env[key])).isFile(), `${key} must name an existing file`);
  }
  requestSource = await readFile(new URL("../../engine/src/bin/cyanrex-platform/session_ui/request.js", import.meta.url));
  directory = await mkdtemp(join(tmpdir(), "cyanrex-request-cdp-browser-")); await chmod(directory, 0o700);
  server = createServer((request, response) => { void serve(request, response).catch(() => {
    violations.push("synthetic server failed"); response.destroy();
  }); });
  server.listen(0, "127.0.0.1"); await once(server, "listening");
  origin = `http://127.0.0.1:${server.address().port}`;
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  context = await chromium.launchPersistentContext(join(directory, "profile"), {
    executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true, ignoreHTTPSErrors: false,
    env: { ...process.env, XDG_DATA_HOME: join(directory, "data"),
      XDG_CONFIG_HOME: join(directory, "config"), XDG_CACHE_HOME: join(directory, "cache") },
  });
});
after(async () => {
  try { await context?.close(); } finally {
    if (server) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
    if (directory) await rm(directory, { recursive: true, force: true });
  }
});

for (const value of cases) test(`mounted transport CDP: ${value.name}`, { timeout: 20_000 }, async () => {
  const method = value.method || "GET", status = value.status || 200;
  const path = method === "PUT" ? `${task}/content` : method === "POST" ? `${task}/text-artifacts` : task;
  const body = { text: value.large ? marker.repeat(16_384) : marker, publicFixture: true };
  current = { ...value, method, status, path, bytes: Buffer.from(JSON.stringify(body)) };
  const start = requests.length, page = await context.newPage(), session = await context.newCDPSession(page);
  const events = [], pageErrors = [];
  let bound = null;
  session.on("Network.requestWillBeSent", event => {
    if (event.request.url === `${origin}${path}` && event.request.method === method) {
      bound = event.requestId; events.push("request");
    }
  });
  session.on("Network.responseReceived", event => { if (event.requestId === bound) events.push("response"); });
  session.on("Network.loadingFinished", event => { if (event.requestId === bound) events.push("finished"); });
  session.on("Network.loadingFailed", event => {
    if (event.requestId === bound) events.push(loadingFailurePart(event, events.includes("response") ? "response" : "request"));
  });
  page.on("pageerror", () => pageErrors.push("synthetic page runtime failed"));
  const observed = await createCdpResponses(session);
  try {
    await page.goto(`${origin}/`); await page.waitForFunction(() => globalThis.fixtureReady === true);
    const reply = observed.expect(`${origin}${path}`, method);
    const ui = page.evaluate(input => globalThis.exercise(input), { mode: value.mode, path, method,
      ...(method === "GET" ? {} : { body: "{}" }) });
    const [network, application] = await Promise.allSettled([reply, ui]);
    // Fulfilled alone is not full-body evidence. Prove every original byte was consumed
    // before classifying a disagreement with the auxiliary observer.
    assert.equal(application.status, "fulfilled"); assert.equal(application.value.status, status);
    assert.equal(application.value.raw, current.bytes.toString("utf8"));
    assert.deepEqual(JSON.parse(application.value.raw), body);
    if (network.status !== "fulfilled") {
      const part = /^[a-z-]+$/.test(network.reason?.observationPart) ? network.reason.observationPart : "unknown";
      throw new Error(`CDP hard failure ${part}; UI fulfilled-complete; phases ${events.join(",")}`);
    }
    assert.deepEqual(network.value, { status, body });
    assert.deepEqual(events, ["request", "response", "finished"]);
    assert.deepEqual(requests.slice(start), [{ path, method, body: method === "GET" ? "" : "{}" }]);
    assert.deepEqual(pageErrors, []); assert.deepEqual(violations, []); observed.check();
  } finally { await observed.close(); await page.close(); current = null; }
});
