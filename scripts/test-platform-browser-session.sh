#!/usr/bin/env bash
set -euo pipefail
# Real SQL/browser acceptance is opt-in; never create storage or change user/system trust here.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
command -v openssl >/dev/null
command -v certutil >/dev/null
browser_session_test_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$browser_session_test_root"
for browser_session_case in \
  plain::postgres_platform_browser_plain_real_session_cookie_otp_confirmation_and_storage \
  tls::postgres_platform_browser_tls_private_ca_real_cookie_session_and_storage; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_session_tdd "$browser_session_case" -- --ignored --list | grep -Fx "$browser_session_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_session_tdd "$browser_session_case" -- --ignored --exact
done
