#!/usr/bin/env bash
set -euo pipefail
# Explicit read-only browser/source evidence; never install, seed or grant from this runner.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
browser_tasks_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$browser_tasks_root"
browser_tasks_case=behavior::postgres_platform_browser_tasks_real_private_pages_reopen_and_revocation_never_write
cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_tasks_tdd "$browser_tasks_case" -- --ignored --list | grep -Fx "$browser_tasks_case: test"
cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_tasks_tdd "$browser_tasks_case" -- --ignored --exact
