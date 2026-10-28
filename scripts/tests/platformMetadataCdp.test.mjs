// Synthetic CDP transport only. These tests do not establish real browser or SQL acceptance.
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { createCdpResponses } from "../../engine/tests/platform_browser_task_metadata/cdp-responses.mjs";
import { createObservations } from "../../engine/tests/platform_browser_task_metadata/observations.mjs";

const url = "http://127.0.0.1:43127/platform/v1/tasks/00000000-0000-0000-0000-00000000002a";
const secret = "sensitive-fixture-Cookie-OTP-and-private-task-74c681";
const value = { schema_version: 1, task: { title: "教师 한국어 📚", revision: 2 } };
const turn = () => new Promise(resolve => setImmediate(resolve));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const events = ["Network.requestWillBeSent", "Network.responseReceived", "Network.loadingFinished", "Network.loadingFailed"];

class Session extends EventEmitter {
  commands = [];
  detachCount = 0;
  detachError = null;
  body = () => ({ body: JSON.stringify(value), base64Encoded: false });
  enabled = () => {};
  async send(method, parameters) {
    this.commands.push({ method, parameters });
    if (method === "Network.enable") { this.enabled(); return {}; }
    if (method === "Network.getResponseBody") return this.body(parameters);
    assert.fail(`unexpected observer command: ${method}`);
  }
  async detach() {
    assert.equal(this.detachCount++, 0, "an owned CDP session must not detach twice");
    if (this.detachError) throw this.detachError;
    this.emit("close", this);
  }
}

function request(session, id = "wanted", target = url, method = "GET", extra = {}) {
  session.emit("Network.requestWillBeSent", { requestId: id,
    request: { url: target, method, headers: { Cookie: secret }, postData: secret }, ...extra });
}
function response(session, id = "wanted", target = url, status = 200) {
  session.emit("Network.responseReceived", { requestId: id,
    response: { url: target, status, headers: { "set-cookie": secret } } });
}
function finished(session, id = "wanted") {
  session.emit("Network.loadingFinished", { requestId: id, encodedDataLength: 77 });
}
function complete(session, id = "wanted", target = url, method = "GET", status = 200) {
  request(session, id, target, method); response(session, id, target, status); finished(session, id);
}
function checkSafe(error, part) {
  assert.ok(error instanceof Error);
  assert.equal(error.observationPart, part);
  assert.doesNotMatch(JSON.stringify({ ...error, message: error.message, stack: error.stack, cause: error.cause }),
    /sensitive-fixture|unredacted-body/);
  return true;
}
const rejectsSafe = (promise, part) => assert.rejects(promise, error => checkSafe(error, part));
function commands(session, ids) {
  assert.deepEqual(session.commands, [{ method: "Network.enable", parameters: undefined },
    ...ids.map(requestId => ({ method: "Network.getResponseBody", parameters: { requestId } }))]);
}

test("CDP listeners precede enable and one exact fast response is read once after finished", async t => {
  const session = new Session();
  session.enabled = () => { for (const event of events) assert.ok(session.listenerCount(event) > 0, event); };
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const pending = replies.expect(url, "GET");
  request(session); response(session);
  assert.equal(session.commands.filter(call => call.method === "Network.getResponseBody").length, 0);
  finished(session);
  assert.deepEqual(await pending, { status: 200, body: value });
  finished(session); response(session); await turn();
  commands(session, ["wanted"]);
});

test("CDP held body remains a required full observation drain barrier", async t => {
  const session = new Session(); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const observations = createObservations(), pending = replies.expect(url, "PUT");
  observations.track("reply-save", () => pending);
  const drained = observations.drain(); complete(session, "wanted", url, "PUT"); await turn();
  assert.equal(typeof release, "function");
  assert.equal(await Promise.race([drained.then(() => "too-early"), turn().then(() => "body-held")]), "body-held");
  release({ body: JSON.stringify(value), base64Encoded: false });
  await drained; assert.deepEqual(await pending, { status: 200, body: value }); commands(session, ["wanted"]);
});

test("CDP base64 response preserves complete multibyte UTF-8 JSON", async t => {
  const session = new Session();
  session.body = () => ({ body: Buffer.from(JSON.stringify(value), "utf8").toString("base64"), base64Encoded: true });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const pending = replies.expect(url, "PUT"); complete(session, "wanted", url, "PUT", 409);
  assert.deepEqual(await pending, { status: 409, body: value }); commands(session, ["wanted"]);
});

test("CDP ignores unrelated IDs and binds repeated URLs to separate explicitly armed requests", async t => {
  const session = new Session();
  session.body = ({ requestId }) => ({ body: JSON.stringify({ requestId }), base64Encoded: false });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const first = replies.expect(url, "GET");
  complete(session, "other-method", url, "POST");
  complete(session, "other-url", `${url}?unexpected=1`);
  response(session, "unknown-id"); finished(session, "unknown-id");
  complete(session, "first"); assert.deepEqual(await first, { status: 200, body: { requestId: "first" } });
  const second = replies.expect(url, "GET");
  finished(session, "first"); response(session, "first");
  complete(session, "second"); assert.deepEqual(await second, { status: 200, body: { requestId: "second" } });
  commands(session, ["first", "second"]);
});

test("CDP distinct pending responses retain their IDs even when completion order is reversed", async t => {
  const session = new Session();
  session.body = ({ requestId }) => ({ body: JSON.stringify({ requestId }), base64Encoded: false });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const first = replies.expect(url, "GET"), second = replies.expect(`${url}/content`, "PUT");
  request(session, "first"); request(session, "second", `${url}/content`, "PUT");
  response(session, "second", `${url}/content`, 401); finished(session, "second");
  response(session, "first"); finished(session, "first");
  assert.deepEqual(await first, { status: 200, body: { requestId: "first" } });
  assert.deepEqual(await second, { status: 401, body: { requestId: "second" } });
  commands(session, ["second", "first"]);
});

test("CDP duplicate arm cannot allow one response to satisfy two expectations", async t => {
  const session = new Session(), replies = await createCdpResponses(session); t.after(() => replies.close());
  const first = replies.expect(url, "GET"), firstRejected = rejectsSafe(first, "event-session-closed");
  await rejectsSafe(async () => replies.expect(url, "GET"), "event-failed");
  await replies.close(); await firstRejected; commands(session, []);
});

const eventFailures = [
  ["reused bound request ID", "event-request-reused", session => { request(session); request(session); }],
  ["duplicate matching request", "event-request-duplicate", session => { request(session); request(session, "duplicate"); }],
  ["redirect on initial match", "event-redirect", session => request(session, "wanted", url, "GET", { redirectResponse: { url, status: 302 } })],
  ["duplicate response", "event-response-duplicate", session => { request(session); response(session); response(session); }],
  ["response URL mismatch", "event-response-mismatch", session => { request(session); response(session, "wanted", `${url}/wrong`); }],
  ["noninteger response status", "event-status-invalid", session => { request(session); response(session, "wanted", url, 200.5); }],
  ["finished before response", "event-finished-before-response", session => { request(session); finished(session); }],
  ["duplicate finished during held body", "event-finished-duplicate", async session => {
    request(session); response(session); finished(session); await turn(); finished(session);
  }, true],
  ["loading failure", "event-loading-failed", session => { request(session); session.emit("Network.loadingFailed", { requestId: "wanted", errorText: secret, canceled: true }); }],
  ["session closure", "event-session-closed", session => session.emit("close", { privateDetails: secret })],
];
for (const [name, part, fail, readsBody = false] of eventFailures) test(`CDP ${name} is a classified private hard failure`, async t => {
  const session = new Session(); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const pending = replies.expect(url, "GET"), observations = createObservations();
  observations.track("reply-task", () => pending);
  const rejected = rejectsSafe(pending, part);
  const ledgerRejected = assert.rejects(observations.drain(), error => {
    assert.equal(error.check, `reply-task-${part}`);
    assert.equal(error.message, "private browser observation failed");
    assert.doesNotMatch(JSON.stringify({ ...error, message: error.message, stack: error.stack }), /sensitive-fixture|unredacted-body/);
    return true;
  });
  try {
    await fail(session); await Promise.all([rejected, ledgerRejected]);
    if (readsBody) assert.equal(typeof release, "function", "second finish must fail while the first body remains held");
  } finally { release?.({ body: JSON.stringify(value), base64Encoded: false }); }
  finished(session); await turn(); commands(session, readsBody ? ["wanted"] : []);
});

const bodyFailures = [
  ["missing resource", () => { throw new Error(`Network.getResponseBody: No resource with given identifier found ${secret}`); }, "body-resource-missing"],
  ["missing data", () => { throw new Error(`Network.getResponseBody: No data found for resource with given identifier ${secret}`); }, "body-resource-missing"],
  ["protocol failure", () => { throw new Error(`Protocol error (Network.getResponseBody): ${secret}`); }, "body-protocol"],
  ["other body failure", () => { throw new Error(secret); }, "body-failed"],
  ["empty body", () => ({ body: "", base64Encoded: false }), "json-invalid"],
  ["malformed JSON", () => ({ body: `unredacted-body-${secret}`, base64Encoded: false }), "json-invalid"],
];
for (const [name, body, part] of bodyFailures) test(`CDP ${name} fails once without replay or private error leakage`, async t => {
  const session = new Session(); session.body = body;
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const rejected = rejectsSafe(replies.expect(url, "GET"), part); complete(session); await rejected;
  finished(session); await turn(); commands(session, ["wanted"]);
});

for (const phase of ["request", "finished", "body"]) test(`CDP ${phase} timeout rejects the whole attempt without retry`, async t => {
  const session = new Session(); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const replies = await createCdpResponses(session, { timeoutMs: 15 }); t.after(() => replies.close());
  const rejected = rejectsSafe(replies.expect(url, "GET"), "event-timeout");
  if (phase !== "request") { request(session); response(session); }
  if (phase === "body") finished(session);
  // Retain a live test-owned timer even if the observer deliberately unrefs its deadline.
  await Promise.all([rejected, delay(35)]);
  release?.({ body: JSON.stringify(value), base64Encoded: false }); await turn();
  finished(session); complete(session, "late"); await turn();
  commands(session, phase === "body" ? ["wanted"] : []);
});

test("CDP delayed private body rejection remains a mandatory sanitized ledger failure", async t => {
  const session = new Session(); let reject;
  session.body = () => new Promise((_, no) => { reject = no; });
  const replies = await createCdpResponses(session); t.after(() => replies.close());
  const observations = createObservations(); observations.track("reply-save", () => replies.expect(url, "PUT"));
  await turn(); complete(session, "wanted", url, "PUT"); await turn();
  reject(new Error(secret)); await turn();
  await assert.rejects(observations.drain(), error => error.check === "reply-save-body-failed"
    && !JSON.stringify({ ...error, message: error.message, stack: error.stack }).includes(secret));
  commands(session, ["wanted"]);
});

test("CDP close rejects held work, removes listeners, and is idempotent", async () => {
  const session = new Session(), replies = await createCdpResponses(session);
  const rejected = rejectsSafe(replies.expect(url, "GET"), "event-session-closed");
  request(session); response(session);
  await replies.close(); await rejected; await replies.close();
  assert.equal(session.detachCount, 1);
  for (const event of events) assert.equal(session.listenerCount(event), 0, event);
  complete(session, "late"); await turn(); commands(session, []);
  await rejectsSafe(async () => replies.expect(url, "GET"), "event-failed");
});

test("CDP session closure rejects pending observations instead of accepting missing replies", async t => {
  const session = new Session(), replies = await createCdpResponses(session); t.after(() => replies.close());
  const rejected = rejectsSafe(replies.expect(url, "GET"), "event-session-closed");
  session.emit("close", { privateDetails: secret }); await rejected; await replies.close();
  assert.equal(session.detachCount, 0); commands(session, []);
});

test("CDP close rejects an in-flight body and ignores its late completion", async () => {
  const session = new Session(); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const replies = await createCdpResponses(session), rejected = rejectsSafe(replies.expect(url, "GET"), "event-session-closed");
  complete(session); await turn(); await replies.close(); await rejected;
  release({ body: JSON.stringify(value), base64Encoded: false }); await turn();
  assert.equal(session.detachCount, 1); commands(session, ["wanted"]);
});

test("CDP detach failure removes listeners and never exposes its private protocol details", async () => {
  const session = new Session(), replies = await createCdpResponses(session);
  session.detachError = new Error(secret);
  await rejectsSafe(replies.close(), "event-failed");
  for (const event of events) assert.equal(session.listenerCount(event), 0, event);
  assert.equal(session.detachCount, 1); commands(session, []);
});

test("CDP enable failure is sanitized and cannot leave active listeners", async () => {
  const session = new Session(); session.enabled = () => { throw new Error(secret); };
  await rejectsSafe(createCdpResponses(session), "event-failed");
  for (const event of events) assert.equal(session.listenerCount(event), 0, event);
  commands(session, []);
});
