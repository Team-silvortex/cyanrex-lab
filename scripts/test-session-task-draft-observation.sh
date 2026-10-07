#!/usr/bin/env bash
set -euo pipefail

# Fault/cancellation fixtures belong only in an explicitly selected disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_observation_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_observation_repo_root"

for draft_observation_pg_case in \
  behavior::postgres_draft_observation_ready_unpolled_and_complete_have_no_unknown_step \
  behavior::postgres_draft_observation_fresh_tokens_cannot_replace_original_even_for_same_owner \
  behavior::postgres_draft_observation_matching_existing_artifact_is_not_a_recovered_receipt \
  behavior::postgres_draft_observation_task_match_and_later_edits_never_change_attempt_progress \
  behavior::postgres_draft_observation_current_artifact_does_not_claim_to_revalidate_earlier_prefix \
  faults::postgres_draft_observation_deferred_artifact_failure_is_not_visible_and_keeps_orphan_blob \
  faults::postgres_draft_observation_missing_task_skips_artifact_namespace_and_later_match_does_not_resume \
  faults::postgres_draft_observation_artifact_and_task_blob_damage_remain_typed_errors \
  lifecycle::postgres_draft_observation_original_session_logout_revocation_and_expiry_still_deny \
  lifecycle::postgres_draft_observation_expiry_during_exact_read_lock_wait_discards_result \
  concurrency::postgres_draft_observation_pending_creator_visibility_does_not_prove_final_outcome \
  concurrency::postgres_draft_observation_cancelled_read_preserves_progress_sql_files_and_pool; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_observation_tdd "$draft_observation_pg_case" -- --ignored --list | grep -Fx "$draft_observation_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_observation_tdd "$draft_observation_pg_case" -- --ignored --exact --nocapture
done
