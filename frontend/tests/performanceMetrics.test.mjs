import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { parsePerformanceMetrics } from "../src/features/settings/performanceMetrics.ts";
import { requestSettings } from "../src/features/settings/settingsRequest.ts";
import { buildHotspotSummary } from "../src/features/settings/hotspots.ts";

const operation = (changes = {}) => ({ total_requests: 10, cache_hits: 9, cache_misses: 1,
  errors: 0, rejected: 0, in_flight: 0, in_flight_peak: 2, avg_duration_ms: 0.25, ...changes });
const metrics = changes => ({ check: operation(changes), completion: operation() });

test("valid metrics preserve integer counters, zero samples and fractional latency", () => {
  assert.deepEqual(parsePerformanceMetrics(metrics()), metrics());
  const zero = Object.fromEntries(Object.keys(operation()).map(key => [key, 0]));
  assert.deepEqual(parsePerformanceMetrics({ check: zero, completion: zero }), { check: zero, completion: zero });
});

test("metrics reject missing operation objects and missing fields", () => {
  for (const value of [null, [], {}, { check: null, completion: operation() }, { check: operation(), completion: [] }])
    assert.throws(() => parsePerformanceMetrics(value));
  for (const key of Object.keys(operation())) {
    const incomplete = operation(); delete incomplete[key];
    assert.throws(() => parsePerformanceMetrics({ check: incomplete, completion: operation() }), key);
  }
});

test("every metrics counter rejects coercions, negative, fractional and unsafe numbers", () => {
  for (const key of Object.keys(operation()).filter(key => key !== "avg_duration_ms"))
    for (const value of ["1", null, true, [], {}, -1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])
      assert.throws(() => parsePerformanceMetrics(metrics({ [key]: value })), key);
});

test("latency accepts finite fractions but rejects invalid or unrepresentable magnitudes", () => {
  for (const value of [0, 0.001, 150.25, Number.MAX_SAFE_INTEGER])
    assert.equal(parsePerformanceMetrics(metrics({ avg_duration_ms: value })).check.avg_duration_ms, value);
  for (const value of ["1", null, false, -0.1, NaN, Infinity, Number.MAX_VALUE])
    assert.throws(() => parsePerformanceMetrics(metrics({ avg_duration_ms: value })));
});

test("overview aggregate counters cannot exceed exact JavaScript integer arithmetic", () => {
  for (const key of ["total_requests", "cache_hits", "cache_misses", "rejected"]) {
    const value = metrics({ [key]: Number.MAX_SAFE_INTEGER }); value.completion[key] = 1;
    assert.throws(() => parsePerformanceMetrics(value), key);
  }
});

test("independently sampled counters are not incorrectly rejected as an atomic snapshot", () => {
  const value = metrics({ total_requests: 0, cache_hits: 1, errors: 2, rejected: 1, in_flight: 3, in_flight_peak: 2 });
  assert.deepEqual(parsePerformanceMetrics(value), value);
});

test("validated metrics copy their display fields and ignore future extension fields", () => {
  const source = metrics(), expected = structuredClone(source);
  source.extra = { ignored: true }; source.check.extra = "ignored";
  const result = parsePerformanceMetrics(source); source.check.total_requests = 800;
  assert.deepEqual(result, expected); assert.notEqual(result.check, source.check);
});

test("accepted limits keep hotspot arithmetic finite", () => {
  const max = Math.floor(Number.MAX_SAFE_INTEGER / 4);
  const parsed = parsePerformanceMetrics({ check: operation({ total_requests: max, cache_hits: max, avg_duration_ms: Number.MAX_SAFE_INTEGER }),
    completion: operation({ total_requests: max, cache_hits: max, avg_duration_ms: Number.MAX_SAFE_INTEGER }) });
  const summary = buildHotspotSummary(parsed, key => key);
  for (const value of Object.values(summary.overall).filter(value => typeof value === "number")) assert.ok(Number.isFinite(value));
});

test("performance decoding composes with one private read and rejects malformed payloads", async t => {
  const calls = [], signal = new AbortController().signal;
  t.mock.method(globalThis, "fetch", (_url, init) => { calls.push(init); return Response.json(calls.length === 1 ? metrics() : { check: null }); });
  assert.deepEqual(await requestSettings("https://engine.invalid/settings/performance", signal, parsePerformanceMetrics), metrics());
  await assert.rejects(requestSettings("https://engine.invalid/settings/performance", signal, parsePerformanceMetrics), /Invalid performance metrics/);
  assert.equal(calls.length, 2);
  for (const call of calls) {
    assert.equal(call.method, "GET"); assert.equal(call.credentials, "include"); assert.equal(call.cache, "no-store");
    assert.equal(call.redirect, "error"); assert.equal(call.body, undefined); assert.equal(call.headers, undefined);
  }
});

test("all four locales contain explicit unavailable and stale metrics notices", () => {
  for (const locale of ["en", "zh_cn", "es", "ja"]) {
    const source = readFileSync(new URL(`../src/i18n/locales/${locale}.ts`, import.meta.url), "utf8");
    for (const key of ["metricsReadFailed", "metricsStale", "metricsStaleLabel"]) assert.ok(source.includes(`${key}: "`), `${locale}.${key}`);
  }
});
