#!/usr/bin/env bash
set -euo pipefail
# Ephemeral CI-only tooling. Production dependencies and real browser trust are untouched.
: "${CI:?This bootstrap is for disposable CI workers only}"
: "${RUNNER_TEMP:?An explicit CI temporary directory is required}"
# Fail before installing anything. Older Chrome ignores XDG NSS isolation, and an
# existing classic NSS directory takes precedence even in supported versions.
if ! browser_ci_chrome="$(command -v google-chrome)"; then
  printf 'Platform browser CI requires an explicit installed Google Chrome executable.\n' >&2
  exit 1
fi
browser_ci_version="$("$browser_ci_chrome" --version 2>/dev/null)"
if [[ ! "$browser_ci_version" =~ ([0-9]+)\. ]]; then
  printf 'Platform browser CI could not verify the installed Chrome major version.\n' >&2
  exit 1
fi
browser_ci_major="${BASH_REMATCH[1]}"
if (( browser_ci_major < 146 )); then
  printf 'Platform browser CI requires Chrome 146 or newer for private XDG NSS trust.\n' >&2
  exit 1
fi
browser_ci_legacy="${HOME:?An existing account directory is required}/.pki/nssdb"
if [[ -e "$browser_ci_legacy" || -L "$browser_ci_legacy" ]]; then
  printf 'Platform browser CI is blocked by legacy user NSS; it will not remove it, change HOME or bypass certificate verification.\n' >&2
  exit 1
fi
browser_ci_root="$(mktemp -d "$RUNNER_TEMP/cyanrex-platform-browser-XXXXXX")"
mkdir -m 700 "$browser_ci_root/data" "$browser_ci_root/config" "$browser_ci_root/cache"
export XDG_DATA_HOME="$browser_ci_root/data"
export XDG_CONFIG_HOME="$browser_ci_root/config"
export XDG_CACHE_HOME="$browser_ci_root/cache"
npm install --prefix "$browser_ci_root" --ignore-scripts --no-audit --no-fund playwright@1.62.1
export CYANREX_PLAYWRIGHT_MODULE="$browser_ci_root/node_modules/playwright/index.mjs"
export CYANREX_CHROMIUM_PATH="$browser_ci_chrome"
printf 'Platform browser runtime: Node %s; %s\n' "$(node --version)" "$browser_ci_version"
sudo apt-get update -qq
sudo apt-get install -y libnss3-tools
node --test scripts/tests/platformMetadataCdp.browser.mjs
bash scripts/test-platform-session-ui-browser.sh
bash scripts/test-platform-task-reader-browser.sh
bash scripts/test-platform-task-metadata-browser.sh
bash scripts/test-platform-browser-session.sh
bash scripts/test-platform-browser-tasks.sh
bash scripts/test-platform-browser-task-metadata.sh
