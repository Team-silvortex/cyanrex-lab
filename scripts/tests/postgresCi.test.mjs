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

test("Rust CI promotes every auth boundary regression plus script/event persistence checks", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const source = await readFile(new URL("../../engine/src/services/auth_service/postgres_boundary_tests.rs", import.meta.url), "utf8");
  const auth = workflow.split("- name: Run real PostgreSQL authentication integration")[1]?.split("\n      - name:")[0] ?? "";
  const names = [...source.matchAll(/async fn (postgres_\w+)\(/g)].map((match) => match[1]);
  assert.equal(names.length, 10, "do not silently drop a boundary test from CI");
  for (const name of names) assert.ok(auth.includes(`services::auth_service::postgres_boundary_tests::${name}`), name);
  const step = workflow.split("- name: Run real PostgreSQL event and script boundaries")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_DB_FALLBACK: "true"/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.ok(step.includes("services::event_bus::tests::deletion::postgres_filtered_delete_preserves_uncached_rows_and_read_flags"));
  assert.ok(step.includes("script_postgres::scripts_use_postgres_without_file_fallback_and_keep_owner_boundaries_after_reload"));
  assert.equal((step.match(/--ignored --list/g) ?? []).length, 2);
  assert.equal((step.match(/grep -Fx/g) ?? []).length, 2);
  assert.equal((step.match(/--ignored --exact --nocapture/g) ?? []).length, 2);
});

test("Rust CI exercises persisted authentication and duplicate registration on the disposable database", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const engine = workflow.split("  engine:\n")[1]?.split("\n  frontend:\n")[0] ?? "";
  const step = engine.split("- name: Run real PostgreSQL authentication integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_DB_FALLBACK: "true"/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.ok(step.includes("services::auth_service::postgres_tests::postgres_registration_persists_accounts_and_serializes_duplicates"));
  for (const kind of ["revoked_session", "expired_session", "deleted_account"]) {
    assert.ok(step.includes(`services::auth_service::postgres_tests::postgres_${kind}_cannot_revive_in_fallback`));
  }
  for (const name of ["logout_write_failure_is_not_success", "logout_zero_row_deletion_is_not_success", "password_write_failure_is_not_success",
    "account_deletion_failure_is_atomic", "password_zero_row_update_is_not_success", "account_zero_row_deletion_is_not_success",
    "account_mutations_normalize_identity_and_persist", "concurrent_password_changes_cannot_overwrite_newer_credentials"]) {
    assert.ok(step.includes(`services::auth_service::postgres_mutation_tests::postgres_${name}`));
  }
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});
