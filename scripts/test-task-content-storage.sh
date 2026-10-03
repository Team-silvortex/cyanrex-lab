#!/usr/bin/env bash
set -euo pipefail

# Schema-3 trusted storage faults require a disposable database, never an existing deployment.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
task_content_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_content_repo_root"

for task_content_pg_case in \
  behavior::postgres_task_content_keeps_32_ordered_exact_pins_and_duplicate_filename_labels \
  behavior::postgres_task_content_round_trips_empty_and_unicode_manifests_without_artifact_storage \
  behavior::postgres_task_content_edits_title_filename_language_and_order_with_one_revision_each \
  behavior::postgres_task_content_owner_scope_and_duplicate_ids_never_widen_trusted_access \
  behavior::postgres_task_content_stale_noop_and_non_draft_changes_have_no_side_effects \
  behavior::postgres_task_content_and_legacy_storage_reject_each_others_namespace_versions \
  faults::postgres_task_content_matching_catalog_definition_tampering_cannot_enter_manual_storage \
  faults::postgres_task_content_installs_only_explicitly_empty_namespaces \
  faults::postgres_task_content_row_event_and_manifest_tampering_fail_closed \
  faults::postgres_task_content_oversized_persisted_json_is_rejected_at_both_record_boundaries \
  faults::postgres_task_content_missing_nullable_or_rls_storage_never_returns_partial_data \
  faults::postgres_task_content_suppressed_rows_or_events_roll_back_the_complete_edit \
  faults::postgres_task_content_consistent_trigger_tampering_is_not_the_requested_write \
  faults::postgres_task_content_deferred_commit_failure_returns_no_success_or_partial_event \
  faults::postgres_task_content_post_write_scope_or_version_drift_rolls_back_every_record \
  faults::postgres_task_content_revision_overflow_never_wraps_or_appends_an_event \
  concurrency::postgres_task_content_duplicate_creation_and_competing_edits_have_one_winner \
  concurrency::postgres_task_content_edit_and_status_race_share_one_revision_head \
  concurrency::postgres_task_content_readers_observe_one_committed_snapshot_during_an_edit \
  concurrency::postgres_task_content_writers_wait_for_metadata_before_admission \
  concurrency::postgres_task_content_cancelled_event_wait_rolls_back_and_restores_the_pool; do
  cargo test --manifest-path engine/Cargo.toml --locked --test task_content_store_tdd "$task_content_pg_case" -- --ignored --list | grep -Fx "$task_content_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test task_content_store_tdd "$task_content_pg_case" -- --ignored --exact --nocapture
done
