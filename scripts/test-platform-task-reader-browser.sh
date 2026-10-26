#!/usr/bin/env bash
set -euo pipefail
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
task_reader_browser_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_reader_browser_root"
node --test scripts/tests/platformTasks.browser.mjs
