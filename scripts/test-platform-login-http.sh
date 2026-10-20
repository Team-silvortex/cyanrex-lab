#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable administration only; runtime roles use their synthetic fixture passwords.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_login_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_login_http_repo_root"

for platform_login_http_case in \
  behavior::postgres_platform_login_http_confirmed_cookie_uses_restricted_issuer_and_stored_digest \
  behavior::postgres_platform_login_http_wrong_missing_and_replayed_credentials_never_set_cookies \
  behavior::postgres_platform_login_http_full_stock_preserves_otp_without_cleanup_or_cookie \
  faults::postgres_platform_login_http_permissions_readonly_and_original_binding_fail_privately \
  lifetime::postgres_platform_login_http_clones_hold_capacity_through_body_and_success_response_lifetimes \
  lifetime::postgres_platform_login_http_cancelled_observed_source_wait_closes_without_manufacturing_success \
  lifetime::postgres_platform_login_http_cancelled_confirmed_but_undelivered_response_keeps_session_and_otp; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_http_tdd "$platform_login_http_case" -- --ignored --list | grep -Fx "$platform_login_http_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_http_tdd "$platform_login_http_case" -- --ignored --exact
done
