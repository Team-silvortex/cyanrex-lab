#!/usr/bin/env bash
set -euo pipefail

# Explicit current-Source SQL and loopback metadata/chunks, never a live app deployment.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
manifest_client_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$manifest_client_repo_root"

manifest_client_cases=(
  source::postgres_granted_client_fetches_real_manifest_and_assembles_published_large_and_empty_files
  source::postgres_granted_client_refuses_revocation_between_manifest_and_chunk_and_revoked_empty_metadata
)
for manifest_client_case in "${manifest_client_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_manifest_client_tdd \
    "$manifest_client_case" -- --ignored --list | grep -Fx "$manifest_client_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_manifest_client_tdd \
    "$manifest_client_case" -- --ignored --exact --nocapture
done
