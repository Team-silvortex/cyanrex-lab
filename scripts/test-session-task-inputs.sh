#!/usr/bin/env bash
set -euo pipefail

# Synthetic multi-namespace authorization faults must never target a deployment database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
task_inputs_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_inputs_repo_root"

for task_inputs_pg_case in \
  postgres_session_task_inputs_pin_exact_versions_and_preserve_input_order \
  postgres_session_task_inputs_are_private_without_teaching_or_deployment_grants \
  postgres_session_task_inputs_reject_missing_foreign_or_mismatched_evidence_atomically \
  postgres_session_task_inputs_keep_manual_and_domain_adapters_strictly_separate \
  postgres_session_task_inputs_bound_evidence_count_and_reject_duplicate_coordinates \
  postgres_session_task_inputs_revision_races_and_terminal_states_have_one_winner \
  lifecycle::postgres_session_task_inputs_missing_membership_and_revoked_sessions_never_expose_bytes \
  lifecycle::postgres_session_task_inputs_retired_and_recreated_accounts_cannot_inherit_evidence \
  faults::postgres_session_task_inputs_artifact_namespace_replacement_preserves_original_pin \
  faults::postgres_session_task_inputs_valid_task_clone_path_cannot_escape_namespace_checks \
  faults::postgres_session_task_inputs_post_write_artifact_corruption_cannot_commit \
  faults::postgres_session_task_inputs_task_event_and_commit_faults_leave_files_unchanged \
  faults::postgres_session_task_inputs_corrupt_files_block_read_and_transition_without_repair \
  faults::postgres_session_task_inputs_post_write_session_deletion_rolls_back_each_mutation \
  concurrency::postgres_session_task_inputs_reader_holds_task_lock_while_waiting_for_artifacts \
  concurrency::postgres_session_task_inputs_reads_original_revision_after_trusted_artifact_writer \
  concurrency::postgres_session_task_inputs_logout_order_preserves_single_transaction_authority \
  concurrency::postgres_session_task_inputs_expiry_after_input_and_event_waits_returns_nothing \
  concurrency::postgres_session_task_inputs_membership_revocation_waits_and_denies_future_access \
  concurrency::postgres_session_task_inputs_cancelled_write_rolls_back_without_touching_input_files; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_inputs_tdd "$task_inputs_pg_case" -- --ignored --list | grep -Fx "$task_inputs_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_inputs_tdd "$task_inputs_pg_case" -- --ignored --exact --nocapture
done
