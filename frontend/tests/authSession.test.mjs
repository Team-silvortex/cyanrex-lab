import assert from "node:assert/strict";
import test from "node:test";
import { logoutSession } from "../src/utils/authSession.ts";

test("logout requires an explicit successful response and uses private non-redirecting transport", async t => {
  let response;
  const requests = [];
  t.mock.method(globalThis, "fetch", async (url, options) => { requests.push({ url, options }); return response; });
  for (const value of [null, {}, { ok: false }, { ok: "true" }, []]) {
    response = Response.json(value);
    await assert.rejects(logoutSession("https://teacher.example"));
  }
  response = Response.json({ ok: true }, { status: 403 });
  await assert.rejects(logoutSession("https://teacher.example"));
  response = new Response("invalid JSON");
  await assert.rejects(logoutSession("https://teacher.example"));
  response = Response.json({ ok: true });
  await logoutSession("https://teacher.example");
  assert.equal(requests.length, 8, "one request per user action; never retry automatically");
  for (const { url, options } of requests) {
    assert.equal(url, "https://teacher.example/auth/logout");
    assert.equal(options.method, "POST");
    assert.equal(options.credentials, "include");
    assert.equal(options.cache, "no-store");
    assert.equal(options.redirect, "error");
    assert.equal(options.body, undefined);
    assert.ok(options.signal instanceof AbortSignal);
  }
});

test("logout propagates network failure without retrying", async t => {
  let calls = 0;
  t.mock.method(globalThis, "fetch", async () => { calls += 1; throw new TypeError("synthetic network failure"); });
  await assert.rejects(logoutSession("https://teacher.example"), /synthetic network failure/);
  assert.equal(calls, 1);
});

test("logout combines navigation cancellation with a bounded request deadline", async t => {
  const deadline = new AbortController();
  let timeout;
  t.mock.method(AbortSignal, "timeout", ms => { timeout = ms; return deadline.signal; });
  t.mock.method(globalThis, "fetch", (_url, { signal }) => new Promise((_resolve, reject) => {
    if (signal.aborted) reject(signal.reason);
    else signal.addEventListener("abort", () => reject(signal.reason), { once: true });
  }));
  const navigation = new AbortController();
  const navigating = logoutSession("https://teacher.example", navigation.signal);
  navigation.abort(new Error("navigation"));
  await assert.rejects(navigating, /navigation/);
  const pending = logoutSession("https://teacher.example");
  deadline.abort(new Error("deadline"));
  await assert.rejects(pending, /deadline/);
  assert.equal(timeout, 10000);
});
