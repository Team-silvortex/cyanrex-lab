import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI selects every prepared stored-password profile SQL regression exactly", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-password-profile.sh", import.meta.url), "utf8");
  const source = await readFile(new URL("../../engine/tests/durable_password_profile_tdd.rs", import.meta.url), "utf8");
  const cases = [...source.matchAll(/async fn (postgres_password_profile_\w+)\(/g)].map(match => match[1]);
  const listed = [...runner.matchAll(/^  (postgres_password_profile_\w+)\b/gm)].map(match => match[1]);
  assert.equal(cases.length, 6, "profile SQL cases need deliberate CI coverage");
  assert.deepEqual(listed.sort(), cases.sort(), "no omissions, duplicates or invented names");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-password-profile\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test durable_password_profile_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
