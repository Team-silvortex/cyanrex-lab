#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable administration only; runtime roles use synthetic fixture passwords.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_loopback_host_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_loopback_host_repo_root"

for platform_loopback_host_case in \
  behavior::postgres_platform_loopback_host_capless_http_login_cookie_query_logout_and_sigterm \
  startup::postgres_platform_loopback_host_first_start_rejects_original_catalog_and_marker_substitution \
  faults::postgres_platform_loopback_host_running_host_rejects_root_and_non_source_marker_drift \
  faults::postgres_platform_loopback_host_failed_logout_readonly_and_missing_delete_preserve_cookies \
  waits::postgres_platform_loopback_host_root_swap_during_observed_startup_marker_wait_never_becomes_ready \
  waits::postgres_platform_loopback_host_killed_observed_startup_wait_leaves_no_backend_or_mutation \
  startup::postgres_platform_loopback_host_occupied_bind_and_failed_storage_never_report_ready \
  waits::postgres_platform_loopback_host_sigterm_drains_observed_session_work_without_compensation; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_loopback_host_tdd "$platform_loopback_host_case" -- --ignored --list | grep -Fx "$platform_loopback_host_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_loopback_host_tdd "$platform_loopback_host_case" -- --ignored --exact
done
