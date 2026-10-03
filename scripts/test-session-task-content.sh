#!/usr/bin/env bash
set -euo pipefail

# Session, namespace and byte corruption fixtures must use only a disposable test database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_content_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_content_repo_root"

for session_content_pg_case in \
  behavior::postgres_session_content_reads_ordered_unicode_revisions_and_empty_tasks \
  behavior::postgres_session_content_metadata_edits_and_status_share_exact_revision_checks \
  behavior::postgres_session_content_private_ownership_scope_and_forged_digests_never_grant_access \
  behavior::postgres_session_content_invalid_new_text_never_creates_or_replaces_a_task \
  behavior::postgres_session_content_schema_three_and_legacy_session_adapters_reject_each_other \
  behavior::postgres_session_content_empty_payload_still_requires_valid_artifact_metadata \
  lifecycle::postgres_session_content_invalid_unbound_and_logged_out_sessions_never_expose_content \
  lifecycle::postgres_session_content_suspended_membership_and_expired_sessions_fail_closed \
  lifecycle::postgres_session_content_retired_and_recreated_accounts_cannot_inherit_saved_content \
  faults::postgres_session_content_corrupt_old_bytes_cannot_be_removed_or_hidden_by_metadata_edits \
  faults::postgres_session_content_trusted_nontext_old_content_is_not_reclassified_as_a_valid_edit \
  faults::postgres_session_content_event_suppression_and_deferred_failure_roll_back_without_deleting_artifacts \
  faults::postgres_session_content_old_new_artifact_and_schema_faults_are_rechecked_after_writes \
  faults::postgres_session_content_consistent_task_tampering_and_session_deletion_cannot_commit \
  namespaces::postgres_session_content_artifact_namespace_clone_cannot_replace_the_original_pin \
  namespaces::postgres_session_content_valid_task_clone_search_path_cannot_publish_a_different_namespace \
  namespaces::postgres_session_content_empty_task_pins_artifact_namespace_before_post_write_revisit \
  concurrency::postgres_session_content_competing_edits_have_one_task_manifest_and_event_winner \
  concurrency::postgres_session_content_opposite_old_new_unions_use_one_artifact_lock_order \
  concurrency::postgres_session_content_expiry_after_task_artifact_event_and_empty_metadata_waits_never_commits \
  concurrency::postgres_session_content_expired_read_wait_returns_no_retained_bytes \
  concurrency::postgres_session_content_logout_order_preserves_one_transaction_authority \
  concurrency::postgres_session_content_membership_revocation_waits_then_denies_future_bytes_and_edits \
  concurrency::postgres_session_content_cancellation_at_event_wait_rolls_back_and_restores_the_source_pool \
  concurrency::postgres_session_content_post_write_blob_changes_are_detected_without_deleting_other_content; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_content_tdd "$session_content_pg_case" -- --ignored --list | grep -Fx "$session_content_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_content_tdd "$session_content_pg_case" -- --ignored --exact --nocapture
done
