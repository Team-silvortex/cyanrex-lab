import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI selects every explicit durable OTP schema SQL regression exactly", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-otp-schema.sh", import.meta.url), "utf8");
  const source = await readFile(new URL("../../engine/tests/durable_otp_schema_tdd.rs", import.meta.url), "utf8");
  const cases = [...source.matchAll(/async fn (postgres_otp_schema_\w+)\(/g)].map(match => match[1]);
  const listed = [...runner.matchAll(/^  (postgres_otp_schema_\w+)\b/gm)].map(match => match[1]);
  assert.equal(cases.length, 4, "schema rejection, initialization and fault cases need deliberate CI coverage");
  assert.equal([...source.matchAll(/#\[ignore\b/g)].length, cases.length);
  assert.deepEqual(listed.sort(), cases.sort(), "no missing, duplicate or invented SQL names");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-otp-schema\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test durable_otp_schema_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
