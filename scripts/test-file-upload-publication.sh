#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL and exclusive private filesystem fixtures only.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
upload_publication_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$upload_publication_repo_root"

for upload_publication_pg_case in \
  baseline::postgres_publication_installer_acknowledges_one_fresh_empty_namespace \
  baseline::postgres_real_confirmed_file_first_publication_returns_receipt_and_persists_metadata \
  cases::behavior::postgres_cross_chunk_empty_publication_and_metadata_reads_are_exact \
  cases::readiness::postgres_confirmed_journal_requires_exact_original_source_root_and_chunks \
  cases::integrity::postgres_confirmed_chunks_still_require_current_bytes_and_whole_digest \
  cases::uniqueness::postgres_upload_version_uniqueness_and_global_capacity_include_empty_files \
  cases::concurrency::postgres_independent_rc_writers_cannot_oversubscribe_publication_capacity \
  cases::faults::postgres_insert_and_deferred_foreign_delta_faults_never_adopt_rows \
  cases::schema::postgres_original_publication_pins_shadow_and_corrupt_foreign_records_fail_closed \
  cases::cancellation::postgres_actual_sql_cancellation_is_terminal_but_does_not_burn_uninserted_coordinate \
  cases::finalization::postgres_independently_visible_ack_then_source_or_journal_drift_is_unconfirmed \
  cases::finalization::postgres_independently_visible_ack_then_original_inode_or_root_drift_is_unconfirmed \
  cases::publication_ack::postgres_independently_visible_ack_then_publication_row_or_schema_drift_is_unconfirmed; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_publication_tdd \
    "$upload_publication_pg_case" -- --ignored --list | grep -Fx "$upload_publication_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_publication_tdd \
    "$upload_publication_pg_case" -- --ignored --exact --nocapture
done
