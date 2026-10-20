#!/usr/bin/env bash
set -euo pipefail

# Disposable administration only; test runtime roles must authenticate by password.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
command -v openssl >/dev/null
platform_loopback_tls_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_loopback_tls_repo_root"

for platform_loopback_tls_case in \
  behavior::postgres_platform_loopback_tls_host_real_https_ipv4_ipv6_secure_cookie_login_query_logout_and_signals \
  behavior::postgres_platform_loopback_tls_host_unknown_ca_and_wrong_peer_refuse_before_login_then_trusted_client_succeeds \
  startup::postgres_platform_loopback_tls_host_tls_bind_and_original_storage_startup_failures_never_report_ready \
  faults::postgres_platform_loopback_tls_host_running_drift_readonly_and_missing_delete_preserve_unconfirmed_cookies \
  waits::postgres_platform_loopback_tls_host_sigterm_drains_observed_sql_work_without_compensation \
  notify::postgres_platform_loopback_host_systemd_ready_then_login_stopping_and_clean_exit \
  notify::postgres_platform_loopback_tls_host_systemd_ready_then_https_login_stopping_and_clean_exit \
  notify::postgres_platform_loopback_tls_host_notify_startup_failure_silent_and_shutdown_send_failure_still_drains; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_loopback_tls_host_tdd "$platform_loopback_tls_case" -- --ignored --list | grep -Fx "$platform_loopback_tls_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_loopback_tls_host_tdd "$platform_loopback_tls_case" -- --ignored --exact
done
