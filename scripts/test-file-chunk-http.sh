#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL/synthetic files and in-process HTTP frames; not LAN delivery.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
chunk_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$chunk_http_repo_root"

for chunk_http_pg_case in \
  baseline::postgres_chunk_http_returns_verified_private_bytes_for_current_exact_recipient \
  behavior::postgres_chunk_http_cross_chunk_selected_only_binary_is_exact_and_private \
  authorization::postgres_chunk_http_current_recipient_lifecycle_and_original_gap_failures_never_emit_bytes \
  storage::postgres_chunk_http_selected_storage_corruption_rejects_without_partial_disclosure_or_repairs \
  response_lifetime::postgres_chunk_http_binary_response_frame_clones_slices_and_unclaimed_results_retain_both_budgets \
  waits::postgres_chunk_http_real_final_wait_expiry_cancel_and_lease_cleanup_discard_private_bytes; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_read_http_tdd \
    "$chunk_http_pg_case" -- --ignored --list | grep -Fx "$chunk_http_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_read_http_tdd \
    "$chunk_http_pg_case" -- --ignored --exact --nocapture
done
