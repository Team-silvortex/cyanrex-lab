#!/usr/bin/env bash
set -euo pipefail

# Use only an explicit disposable administrator fixture; runtime roles authenticate by password.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_storage_open_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_storage_open_repo_root"

for platform_storage_open_case in \
  behavior::postgres_platform_storage_open_uses_declared_binding_for_retained_session_lifecycle \
  identity::postgres_platform_storage_open_never_adopts_replaced_catalog_or_marker_identities \
  admission::postgres_platform_storage_open_configuration_and_raw_root_mismatch_never_dial \
  admission::postgres_platform_storage_open_replaced_original_root_refuses_before_dial \
  waits::postgres_platform_storage_open_rechecks_original_root_after_observed_marker_wait \
  waits::postgres_platform_storage_open_cancelled_actual_marker_wait_leaves_no_binding_or_backend \
  behavior::postgres_platform_storage_open_readonly_minimal_markers_never_install_or_broaden_permissions; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_tdd "$platform_storage_open_case" -- --ignored --list | grep -Fx "$platform_storage_open_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_tdd "$platform_storage_open_case" -- --ignored --exact
done
