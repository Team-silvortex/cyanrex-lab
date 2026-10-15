#!/usr/bin/env bash
set -euo pipefail

# Runtime roles need password authentication; admin-only local trust is permitted.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_storage_connection_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_storage_connection_repo_root"

for platform_storage_connection_case in \
  behavior::postgres_platform_storage_connection_fresh_observed_and_expected_bindings_reject_mismatched_construction \
  behavior::postgres_platform_storage_connection_rejects_marker_replacement_before_first_call_and_later_drift \
  roots::postgres_platform_storage_connection_original_root_precheck_rejects_before_dial_and_never_adopts_replacement \
  roots::postgres_platform_storage_connection_original_root_postcheck_rejects_swap_during_real_marker_wait; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_connection_tdd "$platform_storage_connection_case" -- --ignored --list | grep -Fx "$platform_storage_connection_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_connection_tdd "$platform_storage_connection_case" -- --ignored --exact
done
