import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI selects every explicit bounded Session cleanup transaction exactly", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-session-cleanup.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_cleanup_sql_tests.rs", ""], ["session_cleanup_sql_behavior.rs", "behavior::"], ["session_cleanup_sql_faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/src/services/auth_service/durable_source/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_cleanup_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_cleanup_\w+)\b/gm)].map(match => match[1]);
  assert.equal(cases.length, 9, "cleanup boundaries require deliberate CI coverage");
  assert.deepEqual(listed.sort(), cases.sort(), "no missing, repeated or invented selectors");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-session-cleanup\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /services::auth_service::durable_source::session_cleanup::sql_tests/);
  assert.match(runner, /--locked --lib/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
