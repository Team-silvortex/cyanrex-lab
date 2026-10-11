#!/usr/bin/env bash
set -euo pipefail

# Only disposable PostgreSQL and exclusive private filesystem fixtures; no live Engine composition.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
upload_staging_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$upload_staging_repo_root"

for upload_staging_pg_case in \
  baseline::postgres_upload_bound_chunk_has_exact_local_receipt \
  cases::behavior::postgres_real_cross_chunk_receipts_are_exact_and_attempts_are_single_use \
  cases::binding::postgres_prepare_visibility_generation_scope_and_index_are_read_only \
  cases::binding::postgres_issuer_clone_fixed_destination_and_full_manifest_binding_are_exact \
  cases::pins::postgres_prepared_schema_and_complete_source_record_drift_never_dispatches \
  cases::finalization::postgres_postwrite_real_sql_wait_then_source_drift_is_unconfirmed \
  cases::finalization::postgres_same_bytes_new_inode_after_stage_receipt_never_confirms \
  cases::workers::postgres_control_admission_and_cancelled_real_worker_preserve_attempt_states \
  cases::workers::postgres_input_failures_retain_unknown_files_and_never_adopt_matching_legacy_bytes; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_staging_tdd \
    "$upload_staging_pg_case" -- --ignored --list | grep -Fx "$upload_staging_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_staging_tdd \
    "$upload_staging_pg_case" -- --ignored --exact --nocapture
done
