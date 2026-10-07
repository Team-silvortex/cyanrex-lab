#!/usr/bin/env bash
set -euo pipefail

# Current-Session reads and fault injection belong only in an explicitly disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_inspection_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_inspection_repo_root"

for draft_inspection_pg_case in \
  behavior::postgres_draft_inspection_parsed_checkpoint_survives_attempt_drop_and_source_reopen \
  behavior::postgres_draft_inspection_fresh_session_is_independent_of_original_attempt_fingerprint \
  behavior::postgres_draft_inspection_foreign_owner_cannot_read_the_checkpoint_target \
  behavior::postgres_draft_inspection_metadata_and_labels_compare_only_observable_facts \
  behavior::postgres_draft_inspection_exact_historical_artifact_is_not_a_current_task_receipt \
  faults::postgres_draft_inspection_binary_and_control_artifacts_differ_from_text_checkpoint \
  faults::postgres_draft_inspection_digest_blob_and_namespace_failures_are_typed \
  faults::postgres_draft_inspection_missing_task_skips_artifact_namespace_but_existing_empty_task_checks_it \
  lifecycle::postgres_draft_inspection_state_and_scope_rejections_do_not_enter_source_sql \
  lifecycle::postgres_draft_inspection_current_session_revocation_and_wait_expiry_discard_results \
  concurrency::postgres_draft_inspection_cancelled_read_preserves_checkpoint_sql_files_and_pool \
  concurrency::postgres_draft_inspection_pending_creator_visibility_never_reserves_or_recovers_targets; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_inspection_tdd "$draft_inspection_pg_case" -- --ignored --list | grep -Fx "$draft_inspection_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_inspection_tdd "$draft_inspection_pg_case" -- --ignored --exact --nocapture
done
