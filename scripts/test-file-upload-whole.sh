#!/usr/bin/env bash
set -euo pipefail

# Only explicit disposable PostgreSQL and exclusive private filesystem fixtures.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
upload_whole_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$upload_whole_repo_root"

for upload_whole_pg_case in \
  baseline::postgres_whole_upload_observation_verifies_real_bound_bytes \
  cases::behavior::postgres_cross_chunk_empty_and_readonly_observations_are_exact_nonmutating \
  cases::integrity::postgres_complete_chunks_still_require_whole_digest_and_order \
  cases::filesystem::postgres_missing_corrupt_special_or_nonprivate_chunks_fail_without_repairs \
  cases::binding::postgres_visibility_scope_source_full_manifest_and_legacy_namespace_are_separate \
  cases::finalization::postgres_postio_source_wait_drift_cannot_return_an_observation \
  cases::finalization::postgres_postio_original_files_or_root_replacement_are_unconfirmed \
  cases::workers::postgres_shared_controls_and_existing_staging_worker_admission_fail_closed; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_whole_tdd \
    "$upload_whole_pg_case" -- --ignored --list | grep -Fx "$upload_whole_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_whole_tdd \
    "$upload_whole_pg_case" -- --ignored --exact --nocapture
done
