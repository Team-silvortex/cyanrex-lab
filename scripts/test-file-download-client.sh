#!/usr/bin/env bash
set -euo pipefail

# Explicit current-Source SQL and real HTTP; no normal-app listener, migration or deployment.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
download_client_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$download_client_repo_root"

download_client_case=source::postgres_download_assembles_real_published_bytes_and_stops_at_later_revoked_grant
cargo test --manifest-path engine/Cargo.toml --locked --test file_download_client_tdd \
  "$download_client_case" -- --ignored --list | grep -Fx "$download_client_case: test"
cargo test --manifest-path engine/Cargo.toml --locked --test file_download_client_tdd \
  "$download_client_case" -- --ignored --exact --nocapture
