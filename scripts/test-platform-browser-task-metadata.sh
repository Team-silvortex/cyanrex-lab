#!/usr/bin/env bash
set -euo pipefail
# Explicit original-storage browser evidence. No dependency installs, automatic grants or retries.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
browser_metadata_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$browser_metadata_root"
browser_metadata_case=behavior::postgres_platform_browser_task_metadata_confirmed_replace_conflict_and_revocation
cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_task_metadata_tdd "$browser_metadata_case" -- --ignored --list | grep -Fx "$browser_metadata_case: test"
cargo test --manifest-path engine/Cargo.toml --locked --test platform_browser_task_metadata_tdd "$browser_metadata_case" -- --ignored --exact
