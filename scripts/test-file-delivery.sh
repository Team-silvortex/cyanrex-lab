#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable SQL and actual loopback HTTP/HTTPS delivery, not normal-app startup.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
command -v openssl >/dev/null
delivery_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$delivery_repo_root"

delivery_cases=(
  source::postgres_delivery_fetches_authorized_large_and_empty_bytes_and_saves_new_selected_files
  source::postgres_delivery_revocation_after_manifest_and_revoked_empty_metadata_never_save_targets
  source::distribution_lifetime::postgres_distribution_keeps_binary_owners_and_metadata_independent_across_clones_and_revocation
  source::source_https::acceptance::postgres_openssl_controlled_delivery_saves_authorized_large_short_and_empty_selected_files
  source::source_https::revocation::postgres_openssl_revocation_after_manifest_and_first_chunk_ack_refuses_later_chunk_without_saving
  source::source_https::cancellation::postgres_openssl_explicit_tls_body_cancellation_closes_only_waiter_and_preserves_real_ack_prefix
)
for delivery_case in "${delivery_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tdd \
    "$delivery_case" -- --ignored --list | grep -Fx "$delivery_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tdd \
    "$delivery_case" -- --ignored --exact --nocapture
done
