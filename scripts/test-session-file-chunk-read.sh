#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL and synthetic private files only; not client transport.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
chunk_read_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$chunk_read_repo_root"

for chunk_read_pg_case in \
  baseline::postgres_current_recipient_reads_verified_original_chunk_and_retains_capacity_until_drop \
  behavior::postgres_chunk_read_returns_exact_cross_chunk_bytes_and_rejects_empty_or_local_out_of_range_indices \
  authorization::postgres_chunk_read_requires_current_exact_recipient_grant_without_publisher_or_teacher_bypass \
  lifecycle::postgres_chunk_read_rechecks_current_human_membership_session_and_account_generation \
  filesystem::postgres_chunk_read_rejects_corrupt_linked_special_or_wrong_bound_files_without_disclosure_or_repair \
  configuration::postgres_chunk_read_preserves_source_settings_and_has_no_detached_store_or_upload_journal_dependency \
  records::postgres_chunk_read_rejects_selected_canonical_history_and_hidden_binding_corruption \
  gap::postgres_chunk_read_rejects_original_source_registry_recipient_and_publication_drift_across_gap \
  waits::postgres_chunk_read_final_wait_rechecks_exact_session_and_recipient_expiry_before_bytes_escape \
  cancellation::postgres_chunk_read_cancelled_final_wait_releases_private_buffer_and_sql_locks_without_writes; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_chunk_read_tdd \
    "$chunk_read_pg_case" -- --ignored --list | grep -Fx "$chunk_read_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_chunk_read_tdd \
    "$chunk_read_pg_case" -- --ignored --exact --nocapture
done
