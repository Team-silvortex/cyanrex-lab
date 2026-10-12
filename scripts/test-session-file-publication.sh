#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; never consult an ambient application database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
file_publication_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$file_publication_repo_root"

for session_file_publication_pg_case in \
  baseline::postgres_current_role_free_session_with_exact_active_grant_publishes_real_confirmed_bytes \
  behavior::postgres_session_file_publication_observes_real_cross_chunk_and_empty_bytes_with_one_shot_issuer_state \
  grants::postgres_session_file_publication_requires_current_exact_subject_generation_and_active_half_open_grant \
  lifecycle::postgres_session_file_publication_rechecks_logout_expiry_human_membership_audit_and_account_recreation \
  faults::postgres_session_file_publication_immediate_and_deferred_final_authority_or_ledger_faults_roll_back \
  relations::postgres_session_file_publication_rejects_original_source_registry_grant_upload_journal_and_target_pins \
  waits::postgres_session_file_publication_real_final_marker_wait_rechecks_session_and_grant_window \
  cancellation::postgres_session_file_publication_cancelled_deferred_commit_wait_is_terminal_and_observed_rollback_is_not_retry_authority \
  concurrency::postgres_session_file_publication_queued_rr_writers_use_fresh_retained_quota_and_preserve_readonly_defaults \
  finalization::postgres_session_file_publication_independently_visible_ack_then_original_files_or_records_drift_is_unconfirmed; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_publication_tdd \
    "$session_file_publication_pg_case" -- --ignored --list | grep -Fx "$session_file_publication_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_publication_tdd \
    "$session_file_publication_pg_case" -- --ignored --exact --nocapture
done
