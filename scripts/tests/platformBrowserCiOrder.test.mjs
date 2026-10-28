import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workflowFile = new URL("../../.github/workflows/ci.yml", import.meta.url);
const databaseUrl = 'postgres://cyanrex_test:cyanrex-ci-only@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test';

test("platform browser CI has a required early observation without removing its original late acceptance", async () => {
  const workflow = await readFile(workflowFile, "utf8");
  const engine = workflow.split("\n  engine:\n")[1]?.split("\n  frontend:\n")[0];
  assert.ok(engine, "the Rust engine job remains explicit");
  const names = [...engine.matchAll(/^      - name: (.+)$/gm)].map(match => match[1]);
  const early = names.indexOf("Run early platform browser integration");
  assert.ok(early > names.indexOf("Run tests"));
  assert.ok(early < names.indexOf("Run real PostgreSQL learning integration"));
  const step = name => engine.split(`- name: ${name}\n`)[1]?.split("\n      - name:")[0] ?? "";
  const earlyStep = step("Run early platform browser integration");
  const lateStep = step("Run real PostgreSQL task artifact and review storage integration");
  for (const selected of [earlyStep, lateStep]) {
    assert.ok(selected.includes(`CYANREX_TEST_DATABASE_URL: "${databaseUrl}"`));
    assert.doesNotMatch(selected, /continue-on-error|\bif:|\|\|\s*true|\bretry\b/);
  }
  assert.match(earlyStep, /run: bash scripts\/test-platform-browser-ci\.sh\s*$/);
  assert.match(lateStep, /&& bash scripts\/test-platform-browser-ci\.sh && bash scripts\/test-platform-marker-expectation\.sh/);
  assert.equal([...engine.matchAll(/bash scripts\/test-platform-browser-ci\.sh/g)].length, 2);
});
