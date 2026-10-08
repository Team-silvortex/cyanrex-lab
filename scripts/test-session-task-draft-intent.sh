#!/usr/bin/env bash
set -euo pipefail

# Registration and fault injection require an explicitly disposable PostgreSQL database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_intent_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_intent_repo_root"

for draft_intent_pg_case in \
  behavior::postgres_draft_intent_registration_reopens_ready_metadata_without_publication \
  behavior::postgres_draft_intent_configured_resource_names_need_no_task_or_artifact_schema \
  behavior::postgres_draft_intent_duplicate_registration_across_independent_journal_handles_conflicts \
  lifecycle::postgres_draft_intent_fresh_sessions_require_exact_owner_and_account_generation \
  lifecycle::postgres_draft_intent_only_pristine_original_attempt_may_register \
  lifecycle::postgres_draft_intent_current_session_and_wait_expiry_guard_register_and_read \
  faults::postgres_draft_intent_installation_and_schema_incarnation_are_not_adopted \
  faults::postgres_draft_intent_corrupt_checkpoint_and_routing_metadata_are_rejected \
  faults::postgres_draft_intent_insert_faults_are_hit_and_rolled_back \
  faults::postgres_draft_intent_deferred_commit_failure_never_confirms_registration \
  concurrency::postgres_draft_intent_cancelled_insert_wait_rolls_back_without_publication \
  concurrency::postgres_draft_intent_pending_writer_serializes_read_and_cancelled_observer_is_read_only; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_intent_tdd "$draft_intent_pg_case" -- --ignored --list | grep -Fx "$draft_intent_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_intent_tdd "$draft_intent_pg_case" -- --ignored --exact --nocapture
done
