import assert from "node:assert/strict";
import test from "node:test";
import { buildDetachBody } from "../src/features/ebpf/detachTarget.ts";
import { buildEventDeleteParams } from "../src/features/events/deleteScope.ts";

test("only an explicit null can select detach-all; missing or blank targets fail closed", () => {
  assert.deepEqual(buildDetachBody("/sys/fs/bpf/owned/program"), { pin_path: "/sys/fs/bpf/owned/program" });
  assert.deepEqual(buildDetachBody(null), { pin_path: null });
  for (const value of [undefined, "", "  ", 1, {}]) assert.throws(() => buildDetachBody(value));
});

const now = new Date("2026-09-08T12:00:00Z");
const filters = { categoryFilter: "all", severityFilter: "all", rangePreset: "all", startTime: "", endTime: "" };
test("event deletion freezes an absolute upper bound and never uses the visible list limit", () => {
  const params = buildEventDeleteParams(filters, now);
  assert.deepEqual(Object.fromEntries(params), { end: now.toISOString() });
  const recent = buildEventDeleteParams({ ...filters, rangePreset: "10m", categoryFilter: "kernel", severityFilter: "error" }, now);
  assert.deepEqual(Object.fromEntries(recent), { category: "kernel", severity: "error", start: "2026-09-08T11:50:00.000Z", end: now.toISOString() });
  assert.equal(recent.has("limit"), false);
  assert.equal(recent.has("since_minutes"), false);
});
test("custom deletion keeps an earlier end, caps future ranges and rejects malformed scopes", () => {
  const custom = { ...filters, rangePreset: "custom", startTime: "2026-09-08T09:00:00Z", endTime: "2026-09-08T10:00:00Z" };
  assert.equal(buildEventDeleteParams(custom, now).get("end"), "2026-09-08T10:00:00.000Z");
  assert.equal(buildEventDeleteParams({ ...custom, endTime: "2026-09-09T00:00:00Z" }, now).get("end"), now.toISOString());
  for (const invalid of [{ ...custom, startTime: "invalid" }, { ...custom, endTime: "invalid" },
    { ...custom, startTime: "2026-09-08T11:00:00Z" }, { ...filters, categoryFilter: "unknown" },
    { ...filters, severityFilter: "unknown" }, { ...filters, rangePreset: "unknown" }]) {
    assert.throws(() => buildEventDeleteParams(invalid, now));
  }
});
