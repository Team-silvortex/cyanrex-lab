#!/usr/bin/env bash
set -euo pipefail

# Synthetic admission/authorization faults must never target a deployment database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
catalog_task_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$catalog_task_repo_root"

for catalog_task_pg_case in \
  postgres_session_catalog_tasks_keep_non_teaching_definitions_and_exact_ordered_inputs \
  postgres_session_catalog_tasks_versions_coexist_without_latest_or_missing_pin_fallback \
  postgres_session_catalog_tasks_and_manual_adapters_reject_each_others_definitions \
  postgres_session_catalog_tasks_revision_races_and_terminal_cancellation_preserve_definition \
  lifecycle::postgres_session_catalog_tasks_same_pin_metadata_changes_never_reinterpret_stored_work \
  lifecycle::postgres_session_catalog_tasks_private_membership_never_grants_teacher_access_to_work \
  lifecycle::postgres_session_catalog_tasks_invalid_exact_inputs_never_publish_partial_tasks \
  lifecycle::postgres_session_catalog_tasks_revoked_retired_and_recreated_members_cannot_inherit_work \
  faults::postgres_session_catalog_tasks_post_write_definition_drift_cannot_publish \
  faults::postgres_session_catalog_tasks_post_write_inputs_and_session_faults_roll_back \
  faults::postgres_session_catalog_tasks_artifact_replacement_preserves_original_namespace_pin \
  faults::postgres_session_catalog_tasks_zero_input_post_write_scope_changes_roll_back \
  concurrency::postgres_session_catalog_tasks_reader_holds_task_lock_while_waiting_for_artifacts \
  concurrency::postgres_session_catalog_tasks_expiry_after_metadata_and_event_waits_returns_nothing \
  concurrency::postgres_session_catalog_tasks_logout_order_preserves_single_transaction_authority \
  concurrency::postgres_session_catalog_tasks_cancelled_write_rolls_back_without_touching_inputs; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_task_tdd "$catalog_task_pg_case" -- --ignored --list | grep -Fx "$catalog_task_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_task_tdd "$catalog_task_pg_case" -- --ignored --exact --nocapture
done
