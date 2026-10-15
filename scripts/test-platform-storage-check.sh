#!/usr/bin/env bash
set -euo pipefail

# Runtime roles need password authentication; admin-only local trust is permitted.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_storage_check_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_storage_check_repo_root"

for platform_storage_check_case in \
  behavior::postgres_platform_storage_check_observes_real_markers_and_root_without_mutation_or_leftover_backend \
  failures::postgres_platform_storage_check_rejects_empty_targets_wrong_coordinates_password_and_unsafe_roles \
  failures::postgres_platform_storage_check_rejects_marker_version_scope_permission_and_shape_without_repair \
  waits::postgres_platform_storage_check_rechecks_original_artifact_root_after_real_marker_lock_wait \
  waits::postgres_platform_storage_check_killed_during_real_marker_wait_leaves_no_mutation_or_backend; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_check_tdd "$platform_storage_check_case" -- --ignored --list | grep -Fx "$platform_storage_check_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_check_tdd "$platform_storage_check_case" -- --ignored --exact
done
