#!/usr/bin/env bash
set -euo pipefail

# Fault/cancellation fixtures belong only in an explicitly selected disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_draft_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_draft_repo_root"

for session_draft_pg_case in \
  behavior::postgres_draft_publication_empty_plan_creates_only_one_authorized_task \
  behavior::postgres_draft_publication_orders_unicode_and_same_bytes_using_distinct_server_refs \
  behavior::postgres_draft_publication_unpolled_advance_preserves_ready_without_dispatch \
  behavior::postgres_draft_publication_wrong_token_never_dispatches_and_original_can_continue \
  faults::postgres_draft_publication_second_artifact_failure_preserves_prefix_and_stops \
  faults::postgres_draft_publication_final_commit_failure_never_confirms_task_or_retries \
  faults::postgres_draft_publication_confirmed_blob_damage_prevents_task_without_erasing_receipt \
  faults::postgres_draft_publication_existing_allocated_target_is_conflict_not_adoption \
  cancellation::postgres_draft_publication_cancelled_artifact_wait_keeps_uncertain_target_and_blob \
  cancellation::postgres_draft_publication_cancelled_task_wait_preserves_inputs_without_confirmation \
  lifecycle::postgres_draft_publication_each_step_reauthenticates_logout_and_membership_revocation \
  lifecycle::postgres_draft_publication_final_task_wait_rechecks_database_session_expiry; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_publication_tdd "$session_draft_pg_case" -- --ignored --list | grep -Fx "$session_draft_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_publication_tdd "$session_draft_pg_case" -- --ignored --exact --nocapture
done
