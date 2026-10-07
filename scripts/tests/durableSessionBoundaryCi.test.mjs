import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI enumerates every durable Session namespace and relation boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-session-boundary.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["durable_session_boundary_tdd.rs", ""], ["durable_session_boundary/shapes.rs", "shapes::"], ["durable_session_boundary/writes.rs", "writes::"], ["durable_session_boundary/concurrency.rs", "concurrency::"], ["durable_session_boundary/timestamps.rs", "timestamps::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_boundary_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 17, "source boundary cases require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_boundary_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must select each exact boundary once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-session-boundary\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test durable_session_boundary_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
