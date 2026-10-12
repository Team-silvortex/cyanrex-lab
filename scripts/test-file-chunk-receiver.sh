#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL, synthetic private roots and supplied in-process HTTP bodies.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
chunk_receiver_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$chunk_receiver_repo_root"

for chunk_receiver_pg_case in \
  baseline::postgres_receiver_stages_actual_authorized_http_chunk_into_distinct_private_root \
  bridge::postgres_receiver_stages_both_real_cross_chunk_responses_with_exact_client_manifest_names \
  bridge::postgres_receiver_refuses_real_denied_or_corrupt_http_and_existing_client_chunk_without_retaining_server_leases; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_receiver_tdd \
    "$chunk_receiver_pg_case" -- --ignored --list | grep -Fx "$chunk_receiver_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_receiver_tdd \
    "$chunk_receiver_pg_case" -- --ignored --exact --nocapture
done
