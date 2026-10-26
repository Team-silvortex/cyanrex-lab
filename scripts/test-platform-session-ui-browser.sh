#!/usr/bin/env bash
set -euo pipefail
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
session_ui_test_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_ui_test_root"
node --test scripts/tests/platformSessionUi.browser.mjs
