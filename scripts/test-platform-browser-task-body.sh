#!/usr/bin/env bash
set -euo pipefail
# Explicit original-storage cases. Plain and TLS are separate required selections.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
browser_body_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$browser_body_root"
browser_body_cases=(
  behavior::postgres_platform_browser_task_body_plain_confirmed_replace_conflict_and_revocation
  behavior::postgres_platform_browser_task_body_tls_confirmed_replace_conflict_and_revocation
  partial::postgres_platform_browser_task_body_partial_prefix_second_lock_timeout_retains_confirmed_artifact
  partial::postgres_platform_browser_task_body_partial_prefix_second_session_revoked_retains_confirmed_artifact
)
for browser_body_case in "${browser_body_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_task_body_tdd "$browser_body_case" -- --ignored --list | grep -Fx "$browser_body_case: test"
done
for browser_body_case in "${browser_body_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_task_body_tdd "$browser_body_case" -- --ignored --exact
done
