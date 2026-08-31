import assert from "node:assert/strict";
import test from "node:test";

import { buildHotspotSummary } from "../src/features/settings/hotspots.ts";

const operation = (overrides = {}) => ({
  total_requests: 0,
  cache_hits: 0,
  cache_misses: 0,
  errors: 0,
  rejected: 0,
  in_flight: 0,
  in_flight_peak: 0,
  avg_duration_ms: 0,
  ...overrides,
});
const t = (key) => key;

test("zero-sample metrics are safe instead of false critical hotspots", () => {
  const summary = buildHotspotSummary({ check: operation(), completion: operation() }, t);
  assert.equal(summary.overall.severity, "safe");
  assert.deepEqual(summary.operationHotspots.map((entry) => entry.severity), ["safe", "safe"]);
});

test("real low cache-hit samples remain critical", () => {
  const summary = buildHotspotSummary({
    check: operation({ total_requests: 10, cache_hits: 1, cache_misses: 9 }),
    completion: operation(),
  }, t);
  assert.equal(summary.overall.severity, "critical");
  assert.equal(summary.operationHotspots[0].severity, "critical");
});
