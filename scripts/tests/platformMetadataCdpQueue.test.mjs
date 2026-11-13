// Pure protocol simulation only; this does not establish Chrome or SQL acceptance.
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { createCdpResponses } from "../../engine/tests/platform_browser_task_metadata/cdp-responses.mjs";
import { createObservations } from "../../engine/tests/platform_browser_task_metadata/observations.mjs";

const url = "http://127.0.0.1:43127/platform/v1/tasks/00000000-0000-0000-0000-00000000002a/text-artifacts";
const privateValue = "private-Cookie-OTP-body-and-protocol-text-39bb57";
const turn = () => new Promise(resolve => setImmediate(resolve));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
class Session extends EventEmitter {
  commands = [];
  body = ({ requestId }) => ({ body: JSON.stringify({ ordinal: requestId }), base64Encoded: false });
  async send(method, parameters) {
    this.commands.push({ method, parameters });
    if (method === "Network.enable") return {};
    if (method === "Network.getResponseBody") return this.body(parameters);
    assert.fail("observer must not dispatch another command");
  }
  async detach() { this.emit("close"); }
}
function request(session, id, target = url, method = "POST", extra = {}) {
  session.emit("Network.requestWillBeSent", { requestId: id,
    request: { url: target, method, postData: privateValue, headers: { Cookie: privateValue } }, ...extra });
}
function response(session, id, target = url, status = 201) {
  session.emit("Network.responseReceived", { requestId: id,
    response: { url: target, status, headers: { "set-cookie": privateValue } } });
}
function finish(session, id) { session.emit("Network.loadingFinished", { requestId: id }); }
function complete(session, id, target = url, method = "POST", status = 201) {
  request(session, id, target, method); response(session, id, target, status); finish(session, id);
}
function safe(error, part) {
  assert.equal(error.observationPart, part);
  assert.equal(error.message, "private browser response observation failed");
  assert.doesNotMatch(JSON.stringify({ ...error, message: error.message, stack: error.stack, cause: error.cause }),
    /private-Cookie|OTP-body|protocol-text/);
  return true;
}
const rejected = (promise, part) => assert.rejects(promise, error => safe(error, part));
const rejectAll = (promises, part) => Promise.all(promises.map(promise => rejected(promise, part)));
const reads = session => session.commands.filter(command => command.method === "Network.getResponseBody")
  .map(command => command.parameters.requestId);

test("CDP queue is armed once and binds the next request while the prior body is still held", async t => {
  const session = new Session(), held = new Map();
  session.body = ({ requestId }) => new Promise(resolve => held.set(requestId, resolve));
  const responses = await createCdpResponses(session); t.after(() => responses.close());
  const promises = responses.expectMany(url, "POST", 2), observations = createObservations();
  assert.ok(Object.isFrozen(promises)); assert.equal(promises.length, 2);
  for (const incoming of promises) observations.track("reply-publish", () => incoming);
  const drain = observations.drain();
  complete(session, "first"); await turn();
  complete(session, "second"); await turn();
  assert.deepEqual(reads(session), ["first", "second"]);
  held.get("second")({ body: '{"ordinal":2}', base64Encoded: false });
  assert.deepEqual(await promises[1], { status: 201, body: { ordinal: 2 } });
  assert.equal(await Promise.race([drain.then(() => "early"), turn().then(() => "held")]), "held");
  held.get("first")({ body: '{"ordinal":1}', base64Encoded: false });
  assert.deepEqual(await promises[0], { status: 201, body: { ordinal: 1 } });
  await drain; responses.check(); assert.deepEqual(reads(session), ["first", "second"]);
});

for (const count of [1, 32]) test(`CDP queue accepts its exact inclusive count ${count}`, async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const promises = responses.expectMany(url, "POST", count);
  for (let ordinal = 0; ordinal < count; ordinal++) complete(session, String(ordinal));
  const result = await Promise.all(promises);
  assert.deepEqual(result.map(value => value.body.ordinal), Array.from({ length: count }, (_, i) => String(i)));
  responses.check(); assert.equal(reads(session).length, count);
});

test("CDP queue rejects invalid counts before reserving any slot", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  for (const count of [0, -1, 33, 1.5, NaN, Infinity, "2", null, undefined]) {
    assert.throws(() => responses.expectMany(url, "POST", count), error => safe(error, "event-failed"));
  }
  const [incoming] = responses.expectMany(url, "POST", 1); complete(session, "valid"); await incoming;
  responses.check(); assert.deepEqual(reads(session), ["valid"]);
});

test("CDP queue does not relax existing expect duplicate-arm refusal in either direction", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const pending = responses.expect(url, "POST");
  assert.throws(() => responses.expect(url, "POST"), error => safe(error, "event-failed"));
  assert.throws(() => responses.expectMany(url, "POST", 2), error => safe(error, "event-failed"));
  complete(session, "single"); await pending;
  const queue = responses.expectMany(url, "POST", 2);
  assert.throws(() => responses.expect(url, "POST"), error => safe(error, "event-failed"));
  assert.throws(() => responses.expectMany(url, "POST", 2), error => safe(error, "event-failed"));
  complete(session, "first"); complete(session, "second"); await Promise.all(queue); responses.check();
});

test("CDP interleaved queues and a single PUT retain independent ordered request IDs", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const posts = responses.expectMany(url, "POST", 2), gets = responses.expectMany(url, "GET", 2);
  const put = responses.expect(`${url}/content`, "PUT");
  request(session, "post-1"); request(session, "get-1", url, "GET");
  request(session, "post-2"); request(session, "get-2", url, "GET");
  request(session, "put", `${url}/content`, "PUT");
  for (const id of ["get-2", "post-2", "put", "get-1", "post-1"]) {
    response(session, id, id === "put" ? `${url}/content` : url, id === "put" ? 200 : 201); finish(session, id);
  }
  assert.deepEqual((await Promise.all(posts)).map(value => value.body.ordinal), ["post-1", "post-2"]);
  assert.deepEqual((await Promise.all(gets)).map(value => value.body.ordinal), ["get-1", "get-2"]);
  assert.equal((await put).body.ordinal, "put"); responses.check();
  assert.deepEqual(reads(session), ["get-2", "post-2", "put", "get-1", "post-1"]);
});

test("CDP completed queue guard can only be replaced by an explicit subsequent arm", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const first = responses.expectMany(url, "POST", 1); complete(session, "first"); await Promise.all(first);
  const second = responses.expect(url, "POST"); complete(session, "second"); await second;
  const third = responses.expectMany(url, "POST", 1); complete(session, "third"); await Promise.all(third);
  responses.check(); assert.deepEqual(reads(session), ["first", "second", "third"]);
});

test("CDP queue overrun rejects every still-pending observation without reading extra bodies", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2), other = responses.expect(`${url}/content`, "PUT");
  const failures = rejectAll([...queue, other], "event-request-duplicate");
  request(session, "first"); request(session, "second"); request(session, "extra"); await failures;
  for (const id of ["first", "second", "extra"]) { response(session, id); finish(session, id); }
  assert.throws(() => responses.check(), error => safe(error, "event-request-duplicate"));
  assert.throws(() => responses.expect(`${url}/content`, "PUT"), error => safe(error, "event-failed"));
  assert.throws(() => responses.expectMany(url, "POST", 1), error => safe(error, "event-failed"));
  assert.deepEqual(reads(session), []);
});

test("CDP completed queue still rejects an extra request and prevents a pending PUT confirmation", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 1), other = responses.expect(`${url}/content`, "PUT");
  const failure = rejected(other, "event-request-duplicate");
  complete(session, "first"); await Promise.all(queue);
  request(session, "extra"); await failure;
  complete(session, "put", `${url}/content`, "PUT", 200);
  assert.throws(() => responses.check(), error => safe(error, "event-request-duplicate"));
  assert.deepEqual(reads(session), ["first"]);
});

test("CDP queue check makes a late overrun a mandatory sanitized ledger failure", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 1); complete(session, "first"); await Promise.all(queue);
  request(session, "extra"); const observations = createObservations();
  observations.track("reply-publish", () => responses.check());
  await assert.rejects(observations.drain(), error => error.check === "reply-publish-event-request-duplicate"
    && !JSON.stringify({ ...error, message: error.message, stack: error.stack }).includes(privateValue));
  assert.deepEqual(reads(session), ["first"]);
});

for (const phase of ["bound", "body", "settled"]) test(`CDP queue reused ID at ${phase} never consumes the next slot`, async t => {
  const session = new Session(); let release;
  if (phase === "body") session.body = () => new Promise(resolve => { release = resolve; });
  const responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2);
  const failures = rejectAll(phase === "settled" ? [queue[1]] : queue, "event-request-reused");
  request(session, "same");
  if (phase !== "bound") { response(session, "same"); finish(session, "same"); await turn(); }
  if (phase === "settled") await queue[0];
  request(session, "same"); await failures;
  release?.({ body: '{"late":true}', base64Encoded: false }); await turn();
  assert.throws(() => responses.check(), error => safe(error, "event-request-reused"));
  assert.deepEqual(reads(session), phase === "bound" ? [] : ["same"]);
});

test("CDP queue redirected initial request fails all queues and independent pending work", async t => {
  const session = new Session(), responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2), other = responses.expectMany(`${url}/other`, "GET", 2);
  const failures = rejectAll([...queue, ...other], "event-redirect");
  request(session, "redirected", url, "POST", { redirectResponse: { url: privateValue, status: 307 } });
  await failures; complete(session, "late", `${url}/other`, "GET"); assert.deepEqual(reads(session), []);
});

const brokenEvents = [
  ["duplicate response", "event-response-duplicate", session => { request(session, "first"); response(session, "first"); response(session, "first"); }],
  ["response mismatch", "event-response-mismatch", session => { request(session, "first"); response(session, "first", `${url}/wrong`); }],
  ["invalid status", "event-status-invalid", session => { request(session, "first"); response(session, "first", url, 200.5); }],
  ["finish before response", "event-finished-before-response", session => { request(session, "first"); finish(session, "first"); }],
  ["duplicate finish", "event-finished-duplicate", async session => {
    complete(session, "first"); await turn(); finish(session, "first");
  }, true],
];
for (const [name, part, fail, bodyStarted = false] of brokenEvents) {
  test(`CDP queue ${name} preserves its strict lifecycle failure for all pending work`, async t => {
    const session = new Session(); let release;
    session.body = () => new Promise(resolve => { release = resolve; });
    const responses = await createCdpResponses(session); t.after(() => responses.close());
    const queue = responses.expectMany(url, "POST", 2), other = responses.expect(`${url}/content`, "PUT");
    const failures = rejectAll([...queue, other], part);
    await fail(session); await failures;
    release?.({ body: '{"late":true}', base64Encoded: false }); await turn();
    assert.throws(() => responses.check(), error => safe(error, part));
    assert.deepEqual(reads(session), bodyStarted ? ["first"] : []);
  });
}

for (const stage of ["request", "response", "body"]) {
  test(`CDP queue loadingFailed at ${stage} cannot confirm another held response`, async t => {
    const session = new Session(), held = new Map();
    session.body = ({ requestId }) => new Promise(resolve => held.set(requestId, resolve));
    const responses = await createCdpResponses(session); t.after(() => responses.close());
    const queue = responses.expectMany(url, "POST", 2), other = responses.expect(`${url}/content`, "PUT");
    const part = `event-loading-failed-${stage}`, failures = rejectAll([...queue, other], part);
    complete(session, "first"); request(session, "second");
    if (stage !== "request") response(session, "second");
    if (stage === "body") finish(session, "second");
    await turn();
    // A private suffix cannot be accepted as a known abort or leak its details.
    session.emit("Network.loadingFailed", { requestId: "second", errorText: `net::ERR_ABORTED ${privateValue}` });
    await failures;
    for (const resolve of held.values()) resolve({ body: '{"late":true}', base64Encoded: false });
    await turn();
    assert.throws(() => responses.check(), error => safe(error, part));
    assert.deepEqual(reads(session), stage === "body" ? ["first", "second"] : ["first"]);
  });
}

const brokenBodies = [
  ["private body failure", () => { throw new Error(privateValue); }, "body-failed"],
  ["missing body resource", () => { throw new Error(`Network.getResponseBody: No resource with given identifier ${privateValue}`); }, "body-resource-missing"],
  ["malformed JSON", () => ({ body: privateValue, base64Encoded: false }), "json-invalid"],
  ["invalid body envelope", () => ({ body: {}, base64Encoded: false }), "body-failed"],
];
for (const [name, body, part] of brokenBodies) test(`CDP queue ${name} rejects all pending slots without private leakage`, async t => {
  const session = new Session(); session.body = body;
  const responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2), other = responses.expect(`${url}/content`, "PUT");
  const failures = rejectAll([...queue, other], part);
  complete(session, "first"); await failures;
  complete(session, "second"); complete(session, "put", `${url}/content`, "PUT", 200); await turn();
  assert.throws(() => responses.check(), error => safe(error, part)); assert.deepEqual(reads(session), ["first"]);
});

test("CDP queue timeout invalidates every pending queue slot and unrelated observation", async t => {
  const session = new Session(), responses = await createCdpResponses(session, { timeoutMs: 15 });
  t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2), other = responses.expect(`${url}/content`, "PUT");
  await Promise.all([rejectAll([...queue, other], "event-timeout"), delay(35)]);
  complete(session, "late"); assert.deepEqual(reads(session), []);
  assert.throws(() => responses.check(), error => safe(error, "event-timeout"));
});

test("CDP queue session closure rejects held bodies and slots without accepting late completion", async t => {
  const session = new Session(); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const responses = await createCdpResponses(session); t.after(() => responses.close());
  const queue = responses.expectMany(url, "POST", 2), failures = rejectAll(queue, "event-session-closed");
  complete(session, "first"); await turn(); session.emit("close", privateValue); await failures;
  release({ body: '{"late":true}', base64Encoded: false }); await turn();
  assert.deepEqual(reads(session), ["first"]);
  assert.throws(() => responses.expectMany(url, "POST", 1), error => safe(error, "event-failed"));
});

// Virtual clock only: a sequential batch is not one shared response deadline.
const timed = async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const session = new Session(), responses = await createCdpResponses(session, { timeoutMs: 120 });
  t.after(() => responses.close());
  return { session, responses, tick: milliseconds => t.mock.timers.tick(milliseconds) };
};
test("CDP serial queue gives two 75ms responses independent 120ms budgets", async t => {
  const { session, responses, tick } = await timed(t);
  const queue = responses.expectMany(url, "POST", 2);
  request(session, "first"); tick(75); response(session, "first"); finish(session, "first");
  await queue[0]; request(session, "second"); tick(75);
  response(session, "second"); finish(session, "second");
  assert.deepEqual((await Promise.all(queue)).map(value => value.body.ordinal), ["first", "second"]);
  responses.check(); assert.deepEqual(reads(session), ["first", "second"]);
});

test("CDP queue initial send wait does not consume its bound response budget", async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 1);
  tick(119); request(session, "first"); tick(119); response(session, "first"); finish(session, "first");
  await Promise.all(queue); responses.check(); assert.deepEqual(reads(session), ["first"]);
});

test("CDP successor send wait starts after prior confirmations and has its own response budget", async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 2);
  request(session, "first"); tick(119); response(session, "first"); finish(session, "first"); await queue[0];
  tick(119); request(session, "second"); tick(119); response(session, "second"); finish(session, "second");
  await Promise.all(queue); responses.check(); assert.deepEqual(reads(session), ["first", "second"]);
});

for (const initial of [true, false]) test(`CDP missing ${initial ? "first" : "successor"} request remains bounded`, async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 2);
  let pending = queue;
  if (!initial) {
    request(session, "first"); tick(75); response(session, "first"); finish(session, "first"); await queue[0];
    pending = [queue[1]];
  }
  let outcome;
  const failure = rejectAll(pending, "event-timeout").then(() => { outcome = "failed"; });
  tick(119); await turn(); assert.equal(outcome, undefined); tick(1); await failure;
  assert.throws(() => responses.check(), error => safe(error, "event-timeout"));
  request(session, "late"); response(session, "late"); finish(session, "late");
  assert.deepEqual(reads(session), initial ? [] : ["first"]);
});

test("CDP bound successor times out from its own request and poisons unrelated pending work", async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 2);
  complete(session, "first"); await queue[0]; tick(75); request(session, "second");
  let outcome;
  const other = responses.expect(`${url}/content`, "PUT");
  const failure = rejectAll([queue[1], other], "event-timeout").then(() => { outcome = "failed"; });
  tick(119); await turn(); assert.equal(outcome, undefined); tick(1); await failure;
  response(session, "second"); finish(session, "second");
  assert.throws(() => responses.check(), error => safe(error, "event-timeout"));
  assert.deepEqual(reads(session), ["first"]);
});

test("CDP overlapping bound queue requests retain independent response deadlines", async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 2);
  request(session, "first"); tick(60); request(session, "second"); tick(59);
  response(session, "first"); finish(session, "first"); await queue[0]; tick(60);
  response(session, "second"); finish(session, "second");
  await Promise.all(queue); responses.check(); assert.deepEqual(reads(session), ["first", "second"]);
});

test("CDP queue body retrieval remains inside the bound response deadline", async t => {
  const { session, responses, tick } = await timed(t); let release;
  session.body = () => new Promise(resolve => { release = resolve; });
  const queue = responses.expectMany(url, "POST", 1), failure = rejectAll(queue, "event-timeout");
  tick(75); request(session, "first"); tick(119); response(session, "first"); finish(session, "first");
  await turn(); assert.equal(typeof release, "function"); tick(1); await failure;
  release({ body: '{"late":true}', base64Encoded: false }); await turn();
  assert.throws(() => responses.check(), error => safe(error, "event-timeout"));
  assert.deepEqual(reads(session), ["first"]);
});

test("CDP queue installs a finite conservative whole-batch ceiling and clears it after success", async t => {
  const { session, responses, tick } = await timed(t);
  const schedule = t.mock.method(globalThis, "setTimeout");
  const queue = responses.expectMany(url, "POST", 2);
  assert.ok(schedule.mock.calls.some(call => call.arguments[1] === (2 * 2 + 1) * 120));
  complete(session, "first"); complete(session, "second"); await Promise.all(queue);
  tick(10_000); responses.check();
  const other = responses.expect(`${url}/content`, "PUT"); complete(session, "put", `${url}/content`, "PUT", 200);
  await other; request(session, "extra");
  assert.throws(() => responses.check(), error => safe(error, "event-request-duplicate"));
  assert.deepEqual(reads(session), ["first", "second", "put"]);
});

for (const reason of ["close", "overrun"]) test(`CDP ${reason} cancels queue deadlines without changing its hard failure`, async t => {
  const { session, responses, tick } = await timed(t), queue = responses.expectMany(url, "POST", 2);
  const part = reason === "close" ? "event-session-closed" : "event-request-duplicate";
  const failure = rejectAll(queue, part);
  request(session, "first");
  if (reason === "close") await responses.close();
  else { request(session, "second"); request(session, "extra"); }
  await failure; tick(10_000);
  assert.throws(() => responses.check(), error => safe(error, part));
  assert.deepEqual(reads(session), []);
});

test("CDP single expect keeps its original arm-time budget rather than queue bind-time semantics", async t => {
  const { session, responses, tick } = await timed(t);
  const incoming = responses.expect(url, "POST"), failure = rejected(incoming, "event-timeout");
  tick(75); request(session, "single"); tick(45); await failure;
  response(session, "single"); finish(session, "single"); assert.deepEqual(reads(session), []);
});
