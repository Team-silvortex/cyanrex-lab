import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI explicitly runs read-only lifecycle reconciliation and corruption boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL lifecycle reconciliation integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_reconciliation_tdd.rs", ""], ["durable_reconciliation/faults.rs", "faults::"], ["durable_reconciliation/cli.rs", "cli::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_reconcile_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 12, "reconciliation boundaries require deliberate CI inclusion");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /CYANREX_TEST_DATABASE_PASSWORD: cyanrex-ci-only/);
  assert.match(step, /--test durable_reconciliation_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs local provisioning and secret delivery boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL local provisioning integration")[1]?.split("\n      - name:")[0] ?? "";
  const source = await readFile(new URL("../../engine/tests/provision_postgres_tdd.rs", import.meta.url), "utf8");
  const cases = [...source.matchAll(/async fn (postgres_provision_\w+)\(/g)].map((match) => match[1]);
  assert.equal(cases.length, 9, "provisioning boundaries require deliberate CI inclusion");
  for (const name of cases) assert.ok(step.includes(name), name);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /CYANREX_TEST_DATABASE_PASSWORD: cyanrex-ci-only/);
  assert.match(step, /--test provision_postgres_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every empty-authority bootstrap and rollback boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL authority bootstrap integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_bootstrap_tdd.rs", ""], ["durable_bootstrap/faults.rs", "faults::"], ["durable_bootstrap/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_bootstrap_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
      assert.ok(source.includes(`async fn ${name}() {\n    let _guard = fixture_guard().await;`), `${name} must isolate database-global event hooks`);
    }
  }
  assert.equal(count, 14, "bootstrap boundaries must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test durable_bootstrap_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every atomic password change and revocation boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL password change integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_password_change_tdd.rs", ""], ["durable_password_change/faults.rs", "faults::"], ["durable_password_change/concurrency.rs", "concurrency::"], ["durable_password_change/registry.rs", "registry::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_password_change_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 17, "password/session boundaries must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test durable_password_change_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every account deletion transaction and fault", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL account deletion integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_account_deletion_tdd.rs", ""], ["durable_account_deletion/faults.rs", "faults::"], ["durable_account_deletion/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_deletion_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 16, "deletion boundaries must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test durable_account_deletion_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs atomic session-authorized collaboration commands", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL session-authorized collaboration integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_collaboration_tdd.rs", ""], ["durable_collaboration/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_commands_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 16, "session command boundaries must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test durable_collaboration_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs durable account incarnation and session source boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL durable authentication source integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["durable_auth_source_tdd.rs", ""], ["durable_auth_source/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_source_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 14, "new auth source cases must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test durable_auth_source_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI runs every identity lifecycle command and audit boundary against disposable PostgreSQL", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL collaboration identity audit integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["collaboration_identity_audit_tdd.rs", ""], ["collaboration_identity_audit/permissions.rs", "permissions::"], ["collaboration_identity_audit/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_lifecycle_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 16, "new identity lifecycle cases must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test collaboration_identity_audit_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI runs every attributed policy command and audit fault against disposable PostgreSQL", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL collaboration policy audit integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["collaboration_policy_audit_tdd.rs", ""], ["collaboration_policy_audit/permissions.rs", "permissions::"], ["collaboration_policy_audit/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_audit_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 16, "new policy audit cases must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test collaboration_policy_audit_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI runs every collaboration access policy and revocation case against disposable PostgreSQL", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL collaboration access integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["collaboration_access_store_tdd.rs", ""], ["collaboration_access_store/policy.rs", "policy::"], ["collaboration_access_store/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_access_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 15, "new durable access cases must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test collaboration_access_store_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI runs every collaboration identity persistence case against disposable PostgreSQL", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL collaboration identity integration")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, prefix] of [["collaboration_identity_store_tdd.rs", ""], ["collaboration_identity_store/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_identity_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 15, "new durable identity cases must be deliberately included in CI");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test collaboration_identity_store_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every queued event persistence and retention boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL event ordering and retention")[1]?.split("\n      - name:")[0] ?? "";
  let count = 0;
  for (const [file, module] of [["persistence.rs", "persistence"], ["persistence/followup.rs", "persistence::followup"], ["persistence/cold_start.rs", "persistence::cold_start"], ["persistence/reads.rs", "persistence::reads"], ["persistence/confirmed_mutations.rs", "persistence::confirmed_mutations"], ["persistence/settings.rs", "persistence::settings"]]) {
    const source = await readFile(new URL(`../../engine/src/services/event_bus/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_\w+)\(/g)) {
      count += 1;
      assert.ok(step.includes(`services::event_bus::tests::${module}::${name}`), name);
    }
  }
  assert.equal(count, 51, "new durable cases must be deliberately included in CI");
  assert.match(step, /CYANREX_DB_FALLBACK: "true"/);
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

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
