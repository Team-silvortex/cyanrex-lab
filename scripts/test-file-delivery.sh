#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable SQL and actual loopback delivery, not normal-app startup.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
delivery_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$delivery_repo_root"

delivery_cases=(
  source::postgres_delivery_fetches_authorized_large_and_empty_bytes_and_saves_new_selected_files
  source::postgres_delivery_revocation_after_manifest_and_revoked_empty_metadata_never_save_targets
  source::distribution_lifetime::postgres_distribution_keeps_binary_owners_and_metadata_independent_across_clones_and_revocation
)
for delivery_case in "${delivery_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tdd \
    "$delivery_case" -- --ignored --list | grep -Fx "$delivery_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tdd \
    "$delivery_case" -- --ignored --exact --nocapture
done
