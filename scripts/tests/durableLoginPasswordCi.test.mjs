import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI selects every missing-account password boundary exactly", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-login-password.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["login_password_sql_tests.rs", ""], ["login_password_sql_behavior.rs", "behavior::"]]) {
    const source = await readFile(new URL(`../../engine/src/services/auth_service/durable_source/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/(?:async )?fn (postgres_login_password_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_login_password_\w+)\b/gm)].map(match => match[1]);
  assert.equal(cases.length, 7, "login denial boundaries require deliberate CI coverage");
  assert.deepEqual(listed.sort(), cases.sort(), "no missing, repeated or invented selectors");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-durable-login-password\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /services::auth_service::durable_source::password_work::login_sql_tests/);
  assert.match(runner, /--locked --lib/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
