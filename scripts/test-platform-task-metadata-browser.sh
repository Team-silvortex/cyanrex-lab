#!/usr/bin/env bash
set -euo pipefail
: "${CYANREX_PLAYWRIGHT_MODULE:?Set an explicit absolute Playwright module path}"
: "${CYANREX_CHROMIUM_PATH:?Set an explicit Chromium executable path}"
task_metadata_browser_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_metadata_browser_root"
node --test scripts/tests/platformTaskMetadata.browser.mjs
