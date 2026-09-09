import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI runs PostgreSQL acceptance against its disposable service and rejects an empty selection", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const engine = workflow.split("  engine:\n")[1]?.split("\n  frontend:\n")[0] ?? "";
  assert.match(engine, /services:\n\s+postgres:\n\s+image: postgres:16/);
  assert.match(engine, /"127\.0\.0\.1::5432"/);
  assert.match(engine, /--health-cmd "pg_isready -U cyanrex_test -d cyanrex_test"/);
  const step = engine.split("- name: Run real PostgreSQL learning integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_DB_FALLBACK: "true"/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.ok(step.includes("services::learning_store::feedback::tests::postgres_feedback_migrates_legacy_rows_and_serializes_updates"));
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});
