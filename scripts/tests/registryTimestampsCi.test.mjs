import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("focused registry timestamp runner selects all ten SQL boundaries with exact targets", async () => {
  const runner = await readFile(new URL("../test-registry-timestamps.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [target, file, module] of [
    ["collaboration_identity_store_tdd", "collaboration_identity_store/timestamps.rs", "timestamps"],
    ["collaboration_identity_audit_tdd", "collaboration_identity_audit/timestamps.rs", "timestamps"],
    ["collaboration_policy_audit_tdd", "collaboration_policy_audit/timestamps.rs", "timestamps"],
    ["durable_reconciliation_tdd", "durable_reconciliation/registry_timestamps.rs", "registry_timestamps"],
  ]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    const root = await readFile(new URL(`../../engine/tests/${target}.rs`, import.meta.url), "utf8");
    assert.ok(root.includes(`mod ${module};`), `${target} must mount ${module}`);
    for (const [, name] of source.matchAll(/async fn (postgres_\w+)\(/g)) cases.push(`${target}|${module}::${name}`);
  }
  const listed = [...runner.matchAll(/^  '([a-z_]+\|(?:\w+::)*postgres_\w+)'/gm)].map(match => match[1]);
  assert.equal(cases.length, 10);
  assert.deepEqual(listed.sort(), cases.sort(), "no missing, repeated or wrong-target selectors");
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b|\beval\b/);
  assert.match(runner, /IFS='\|' read -r registry_timestamps_target registry_timestamps_name/);
  assert.match(runner, /--locked --test "\$registry_timestamps_target" "\$registry_timestamps_name"/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});
