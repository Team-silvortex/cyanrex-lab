import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = path => readFile(new URL(`../../${path}`, import.meta.url), "utf8");

test("private feature requests share transport without Runner borrowing Settings", async () => {
  for (const feature of ["settings/settingsRequest", "events/eventRequest", "ebpf/runtimeRequest"]) {
    const source = await read(`frontend/src/features/${feature}.ts`);
    assert.match(source, /from "\.\.\/\.\.\/transport\/privateRequest"/);
    assert.doesNotMatch(source, /\bfetch\s*\(|new AbortController\(|\bsetTimeout\s*\(/);
  }
  for (const feature of ["runner/useRunnerAgentAdmin", "settings/usePerformanceMetrics"]) {
    const source = await read(`frontend/src/features/${feature}.ts`);
    assert.match(source, /from "\.\.\/\.\.\/transport\/privateRequest"/);
    assert.doesNotMatch(source, /from "(?:\.\.\/settings\/|\.\/)?settingsRequest"/);
  }
  const manifest = JSON.parse(await read("frontend/package.json"));
  assert.match(manifest.scripts["test:private-request"], /--test tests\/privateRequest\.test\.mjs/);
  assert.match(await read("scripts/quality-gate.sh"), /run test:private-request/);
  assert.match(await read(".github/workflows/ci.yml"), /npm run test:private-request/);
});

test("shared prepared resource SQL fixtures are explicitly enumerated in CI", async () => {
  const helper = await read("engine/src/services/prepared_resource_sql.rs");
  const cases = [...helper.matchAll(/async fn (postgres_\w+)\(/g)]
    .map(([, name]) => `services::prepared_resource_sql::tests::${name}`).sort();
  assert.equal(cases.length, 2);
  const runner = await read("scripts/test-prepared-resource-sql.sh");
  const listed = [...runner.matchAll(/^  (services::prepared_resource_sql::tests::postgres_\w+)\b/gm)]
    .map(([, name]) => name).sort();
  assert.deepEqual(listed, cases);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--lib/); assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/); assert.match(runner, /--ignored --exact --nocapture/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run shared prepared resource SQL regressions")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /run: bash scripts\/test-prepared-resource-sql\.sh/);
});
