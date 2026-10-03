#!/usr/bin/env bash
set -euo pipefail

# Synthetic replacement/authorization faults must never target a deployment database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
task_revision_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_revision_repo_root"

for task_revision_pg_case in \
  behavior::postgres_session_task_revision_replaces_exact_inputs_without_rewriting_artifacts_or_reviews \
  behavior::postgres_session_task_revision_catalog_zero_inputs_preserve_full_definition_and_adapter_policy \
  behavior::postgres_session_task_revision_stale_replay_and_ordered_noop_are_not_successes \
  behavior::postgres_session_task_revision_invalid_inputs_and_other_owners_never_mutate_the_task \
  behavior::postgres_session_task_revision_accepts_disjoint_32_by_32_union_without_losing_input_order \
  behavior::postgres_session_task_revision_storage_v2_rejects_legacy_v1_without_adopting_or_migrating \
  lifecycle::postgres_session_task_revision_non_draft_and_terminal_tasks_reject_replacement \
  lifecycle::postgres_session_task_revision_invalid_expired_revoked_and_unbound_sessions_fail_closed \
  lifecycle::postgres_session_task_revision_retired_and_recreated_accounts_cannot_inherit_old_tasks \
  faults::postgres_session_task_revision_missing_or_corrupt_old_and_new_blobs_cannot_be_dropped_away \
  faults::postgres_session_task_revision_event_failure_rolls_back_but_keeps_published_new_artifact \
  faults::postgres_session_task_revision_rechecks_removed_and_added_artifact_records_after_task_event \
  faults::postgres_session_task_revision_task_tamper_and_session_deletion_cannot_escape_final_checks \
  concurrency::postgres_session_task_revision_two_writers_have_one_revision_and_outbox_winner \
  concurrency::postgres_session_task_revision_waiting_for_old_task_lock_observes_fresh_revision \
  concurrency::postgres_session_task_revision_expiry_after_task_artifact_and_event_waits_never_commits \
  concurrency::postgres_session_task_revision_cancellation_at_event_wait_rolls_back_and_restores_pool; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_revision_tdd "$task_revision_pg_case" -- --ignored --list | grep -Fx "$task_revision_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_revision_tdd "$task_revision_pg_case" -- --ignored --exact --nocapture
done
