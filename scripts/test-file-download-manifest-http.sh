#!/usr/bin/env bash
set -euo pipefail

# Disposable Source SQL only; metadata observations are not byte authority or live mounting.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
manifest_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$manifest_http_repo_root"

for manifest_http_case in \
  authorization::postgres_manifest_http_maps_current_session_membership_and_workspace_lifecycle_privately \
  authorization::postgres_manifest_http_requires_exact_current_recipient_and_rechecks_revocation_after_body_wait \
  baseline::postgres_manifest_http_returns_complete_private_published_manifest_for_current_exact_recipient \
  behavior::postgres_manifest_http_preserves_empty_and_cross_chunk_metadata_without_publisher_permission_dependency \
  lifetime::postgres_manifest_http_sql_wait_cancel_and_json_response_lifetime_do_not_create_byte_leases_or_fresh_authority \
  storage::postgres_manifest_http_sanitizes_selected_corruption_and_original_pinned_configuration_errors \
  storage::postgres_manifest_http_uses_only_original_source_transaction_without_detached_store_or_file_reads \
  waits::postgres_manifest_http_uses_final_source_clock_after_real_selected_publication_wait; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_download_manifest_http_tdd \
    "$manifest_http_case" -- --ignored --list | grep -Fx "$manifest_http_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_download_manifest_http_tdd \
    "$manifest_http_case" -- --ignored --exact --nocapture
done
