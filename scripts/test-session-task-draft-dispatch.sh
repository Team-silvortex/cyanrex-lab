#!/usr/bin/env bash
set -euo pipefail

# Fault injection requires an explicitly disposable PostgreSQL database, never a deployment URL.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_dispatch_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_dispatch_repo_root"

for draft_dispatch_pg_case in \
  behavior::postgres_draft_dispatch_unicode_steps_commit_exact_resources_and_markers \
  behavior::postgres_draft_dispatch_empty_plan_works_through_independent_source_and_journal_handles \
  behavior::postgres_draft_dispatch_missing_registration_and_wrong_token_never_publish \
  behavior::postgres_draft_dispatch_schema_one_is_rejected_without_migration \
  lifecycle::postgres_draft_dispatch_owner_and_account_generation_are_rechecked_in_each_phase \
  lifecycle::postgres_draft_dispatch_current_session_logout_and_revocation_stop_each_phase \
  faults::postgres_draft_dispatch_phase_a_faults_never_reach_resource_writes \
  faults::postgres_draft_dispatch_artifact_failure_preserves_unknown_and_confirmed_prefix \
  faults::postgres_draft_dispatch_task_failure_preserves_all_confirmed_artifacts \
  faults::postgres_draft_dispatch_phase_b_marker_and_intent_changes_roll_back_resources \
  faults::postgres_draft_dispatch_damaged_marker_shape_and_prefix_are_not_retried \
  concurrency::postgres_draft_dispatch_cancelled_phase_a_cannot_publish_or_retry \
  concurrency::postgres_draft_dispatch_cancelled_phase_b_keeps_unknown_without_resource_commit \
  concurrency::postgres_draft_dispatch_waiting_phase_a_and_b_recheck_database_expiry; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_dispatch_tdd "$draft_dispatch_pg_case" -- --ignored --list | grep -Fx "$draft_dispatch_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_dispatch_tdd "$draft_dispatch_pg_case" -- --ignored --exact --nocapture
done
