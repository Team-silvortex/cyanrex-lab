import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("Rust CI explicitly runs every prepared Task content HTTP transaction boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-platform-task-content-http.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["platform_http_tdd.rs", ""], ["platform_http/behavior.rs", "behavior::"], ["platform_http/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_platform_http_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 11, "HTTP transaction boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_platform_http_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-platform-task-content-http\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test platform_http_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("prepared Task content HTTP does not claim live Engine OpenAPI or SDK integration", async () => {
  for (const file of ["engine/src/application.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts", "sdk-js/src/generated/openapi.ts", "sdk-js/src/generated/operations.ts"]) {
    const source = await readFile(new URL(`../../${file}`, import.meta.url), "utf8");
    assert.doesNotMatch(source, /\/platform\/v1\/tasks|build_task_content_router|TaskContentHttpState/, file);
  }
  const router = await readFile(new URL("../../engine/src/platform_http/mod.rs", import.meta.url), "utf8");
  assert.match(router, /pub fn build_task_content_router/);
  assert.match(router, /\/platform\/v1\/tasks/);
  assert.doesNotMatch(router, /install_empty\s*\(|\bAuthService\s*[:,{]|std::env/);
});

test("Rust CI explicitly runs every Session content and exact-byte boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-task-content.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_task_content_tdd.rs", ""], ["session_task_content/behavior.rs", "behavior::"], ["session_task_content/lifecycle.rs", "lifecycle::"], ["session_task_content/faults.rs", "faults::"], ["session_task_content/namespaces.rs", "namespaces::"], ["session_task_content/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_content_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 25, "Session content boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_content_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-task-content\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_task_content_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every schema-3 Task content atomic storage boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-task-content-storage.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["task_content_store_tdd.rs", ""], ["task_content_store/behavior.rs", "behavior::"], ["task_content_store/faults.rs", "faults::"], ["task_content_store/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_task_content_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 21, "task content storage boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_task_content_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-task-content-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test task_content_store_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every Session Task input replacement boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-task-revisions.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_task_revision_tdd.rs", ""], ["session_task_revision/behavior.rs", "behavior::"], ["session_task_revision/lifecycle.rs", "lifecycle::"], ["session_task_revision/faults.rs", "faults::"], ["session_task_revision/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_task_revision_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 17, "input replacement boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_task_revision_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-task-revisions\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_task_revision_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every Session catalogue Task admission boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-catalog-tasks.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_catalog_task_tdd.rs", ""], ["session_catalog_task/lifecycle.rs", "lifecycle::"], ["session_catalog_task/faults.rs", "faults::"], ["session_catalog_task/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_catalog_tasks_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 16, "catalogue admission boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_catalog_tasks_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-catalog-tasks\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_catalog_task_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every private Session Review and exact evidence boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-review-storage.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_review_tdd.rs", ""], ["session_review/lifecycle.rs", "lifecycle::"], ["session_review/faults.rs", "faults::"], ["session_review/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_reviews_\w+)\(/g)) cases.push(`${prefix}${name}`);
  }
  assert.equal(cases.length, 18, "authorized Review boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_reviews_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-review-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_review_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every authorized Task and exact Artifact input boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-task-inputs.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_task_inputs_tdd.rs", ""], ["session_task_inputs/lifecycle.rs", "lifecycle::"], ["session_task_inputs/faults.rs", "faults::"], ["session_task_inputs/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_task_inputs_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 20, "authorized input boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_task_inputs_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate every exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-task-inputs\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_task_inputs_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every session-authorized artifact and publication boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-artifact-storage.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_artifact_tdd.rs", ""], ["session_artifact/lifecycle.rs", "lifecycle::"], ["session_artifact/faults.rs", "faults::"], ["session_artifact/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_artifacts_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 18, "session/artifact boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_artifacts_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate each exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-artifact-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_artifact_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs every session-authorized task and transaction boundary", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-session-task-storage.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["session_task_tdd.rs", ""], ["session_task/faults.rs", "faults::"], ["session_task/concurrency.rs", "concurrency::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_session_tasks_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 17, "session/task boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_session_tasks_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort(), "runner must enumerate each exact case once");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-session-task-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test session_task_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs review history, judgment and cross-store boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-review-storage.sh", import.meta.url), "utf8");
  let count = 0;
  for (const [file, prefix] of [["review_store_tdd.rs", ""], ["review_store/faults.rs", "faults::"], ["review_store/integration.rs", "integration::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_reviews_\w+)\(/g)) {
      count += 1;
      assert.ok(runner.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 17, "review boundaries require deliberate CI inclusion");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-review-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test review_store_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs durable task, revision and outbox boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-task-storage.sh", import.meta.url), "utf8");
  let count = 0;
  for (const [file, prefix] of [["task_store_tdd.rs", ""], ["task_store/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_tasks_\w+)\(/g)) {
      count += 1;
      assert.ok(runner.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 15, "task storage boundaries require deliberate CI inclusion");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /run: bash scripts\/test-task-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test task_store_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs immutable artifact, file and publication boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-artifact-storage.sh", import.meta.url), "utf8");
  let count = 0;
  for (const [file, prefix] of [["artifact_store_tdd.rs", ""], ["artifact_store/faults.rs", "faults::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_artifacts_\w+)\(/g)) {
      count += 1;
      assert.ok(runner.includes(`${prefix}${name}`), name);
    }
  }
  assert.equal(count, 17, "artifact boundaries require deliberate CI inclusion");
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /&& bash scripts\/test-artifact-storage\.sh/);
  assert.match(runner, /set -euo pipefail/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test artifact_store_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
});

test("Rust CI explicitly runs read-only lifecycle reconciliation and corruption boundaries", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL lifecycle reconciliation integration")[1]?.split("\n      - name:")[0] ?? "";
  const runner = await readFile(new URL("../test-durable-reconciliation.sh", import.meta.url), "utf8");
  const cases = [];
  for (const [file, prefix] of [["durable_reconciliation_tdd.rs", ""], ["durable_reconciliation/faults.rs", "faults::"], ["durable_reconciliation/cli.rs", "cli::"], ["durable_reconciliation/timestamps.rs", "timestamps::"], ["durable_reconciliation/registry_timestamps.rs", "registry_timestamps::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 17, "reconciliation boundaries require deliberate CI inclusion");
  const listed = [...runner.matchAll(/^  ((?:\w+::)*postgres_\w+)\b/gm)].map((match) => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /CYANREX_TEST_DATABASE_PASSWORD: cyanrex-ci-only/);
  assert.match(step, /run: bash scripts\/test-durable-reconciliation\.sh/);
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--test durable_reconciliation_tdd/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /grep -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
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
  const cases = [];
  for (const [file, prefix] of [["collaboration_identity_audit_tdd.rs", ""], ["collaboration_identity_audit/permissions.rs", "permissions::"], ["collaboration_identity_audit/faults.rs", "faults::"], ["collaboration_identity_audit/timestamps.rs", "timestamps::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_lifecycle_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 18, "new identity lifecycle cases must be deliberately included in CI");
  const listed = [...step.matchAll(/^\s+((?:\w+::)*postgres_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
  assert.ok(step.includes("@127.0.0.1:${{ job.services.postgres.ports[5432] }}/cyanrex_test"));
  assert.match(step, /--test collaboration_identity_audit_tdd/);
  assert.match(step, /--ignored --list/);
  assert.match(step, /grep -Fx/);
  assert.match(step, /--ignored --exact --nocapture/);
});

test("Rust CI runs every attributed policy command and audit fault against disposable PostgreSQL", async () => {
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  const step = workflow.split("- name: Run real PostgreSQL collaboration policy audit integration")[1]?.split("\n      - name:")[0] ?? "";
  const cases = [];
  for (const [file, prefix] of [["collaboration_policy_audit_tdd.rs", ""], ["collaboration_policy_audit/permissions.rs", "permissions::"], ["collaboration_policy_audit/faults.rs", "faults::"], ["collaboration_policy_audit/timestamps.rs", "timestamps::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 19, "new policy audit cases must be deliberately included in CI");
  const listed = [...step.matchAll(/^\s+((?:\w+::)*postgres_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
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
  const cases = [];
  for (const [file, prefix] of [["collaboration_identity_store_tdd.rs", ""], ["collaboration_identity_store/faults.rs", "faults::"], ["collaboration_identity_store/timestamps.rs", "timestamps::"]]) {
    const source = await readFile(new URL(`../../engine/tests/${file}`, import.meta.url), "utf8");
    for (const [, name] of source.matchAll(/async fn (postgres_identity_\w+)\(/g)) {
      cases.push(`${prefix}${name}`);
    }
  }
  assert.equal(cases.length, 17, "new durable identity cases must be deliberately included in CI");
  const listed = [...step.matchAll(/^\s+((?:\w+::)*postgres_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(listed.sort(), cases.sort());
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
