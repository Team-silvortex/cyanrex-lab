#!/usr/bin/env bash
set -euo pipefail
# Explicit offline execution only: no build, pull, installation or host trust repair.
browser_container_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
browser_container_mode=preflight
browser_container_image=
browser_container_node=
browser_container_chrome=
browser_container_playwright=
browser_container_session=
browser_container_body=
usage() {
  printf '%s\n' 'Usage: test-platform-browser-container.sh --image LOCAL_IMAGE --node-dir ABS_NODE22_ARCHIVE_DIR --chrome-dir ABS_CHROME_DIR --playwright-dir ABS_PLAYWRIGHT_PACKAGE --session-test ABS_COMPILED_SESSION_TEST --body-test ABS_COMPILED_BODY_TEST [--run-tls]' >&2
  exit 2
}
while (( $# )); do
  case "$1" in
    --image|--node-dir|--chrome-dir|--playwright-dir|--session-test|--body-test)
      (( $# >= 2 )) || usage
      case "$1" in
        --image) browser_container_image="$2" ;;
        --node-dir) browser_container_node="$2" ;;
        --chrome-dir) browser_container_chrome="$2" ;;
        --playwright-dir) browser_container_playwright="$2" ;;
        --session-test) browser_container_session="$2" ;;
        --body-test) browser_container_body="$2" ;;
      esac
      shift 2 ;;
    --run-tls)
      [[ "$browser_container_mode" == preflight ]] || usage
      browser_container_mode=tls; shift ;;
    *) usage ;;
  esac
done
[[ "$browser_container_image" =~ ^[a-z0-9][a-z0-9_./:-]*$ ]] || usage
checked_path() {
  local candidate="$1"
  [[ "$candidate" == /* && "$candidate" != *$'\n'* && "$candidate" != *,* ]] || usage
  readlink -e -- "$candidate"
}
browser_container_node="$(checked_path "$browser_container_node")"
browser_container_root="$(checked_path "$browser_container_root")"
browser_container_chrome="$(checked_path "$browser_container_chrome")"
browser_container_playwright="$(checked_path "$browser_container_playwright")"
browser_container_session="$(checked_path "$browser_container_session")"
browser_container_body="$(checked_path "$browser_container_body")"
browser_container_core="$(checked_path "$browser_container_playwright/../playwright-core")"
[[ "$(basename "$browser_container_node")" =~ ^node-v22\.[0-9]+\.[0-9]+-linux-x64$ ]] || usage
[[ "$(basename "$browser_container_playwright")" == playwright ]] || usage
[[ -f "$browser_container_playwright/index.mjs" && -f "$browser_container_playwright/package.json" && -f "$browser_container_core/package.json" ]] || usage
[[ -f "$browser_container_chrome/resources.pak" && -d "$browser_container_chrome/locales" ]] || usage
[[ "$(dirname "$browser_container_session")" == "$browser_container_root/engine/target/debug/deps" && "$(basename "$browser_container_session")" =~ ^platform_browser_session_tdd-[a-f0-9]+$ ]] || usage
[[ "$(dirname "$browser_container_body")" == "$browser_container_root/engine/target/debug/deps" && "$(basename "$browser_container_body")" =~ ^platform_browser_task_body_tdd-[a-f0-9]+$ ]] || usage
for browser_container_executable in "$browser_container_node/bin/node" "$browser_container_chrome/chrome" \
  "$browser_container_session" "$browser_container_body" "$browser_container_root/engine/target/debug/cyanrex-platform"; do
  [[ -f "$browser_container_executable" && -x "$browser_container_executable" ]] || {
    printf 'An explicit executable test or browser tool is unavailable.\n' >&2; exit 2;
  }
done
[[ -f "$browser_container_root/scripts/platform-browser-container-entry.sh" ]] || usage
command -v docker >/dev/null
docker image inspect "$browser_container_image" >/dev/null
browser_container_name="cyanrex-browser-tests-${BASHPID}-${RANDOM}"
# These are five exact read-only mounts, not a HOME, system library or trust overlay.
docker run --rm --init --pull never --name "$browser_container_name" \
  --label org.cyanrex.purpose=disposable-browser-tls-test \
  --user ubuntu:ubuntu --network none --read-only --cap-drop ALL --security-opt no-new-privileges \
  --memory 3g --memory-swap 3g --pids-limit 512 --cpus 2 --shm-size 256m \
  --tmpfs /tmp:rw,nosuid,nodev,size=4g,mode=1777 \
  --mount "type=bind,src=$browser_container_root,dst=$browser_container_root,readonly" \
  --mount "type=bind,src=$browser_container_node,dst=/opt/cyanrex-browser/node,readonly" \
  --mount "type=bind,src=$browser_container_chrome,dst=/opt/google/chrome,readonly" \
  --mount "type=bind,src=$browser_container_playwright,dst=/opt/cyanrex-browser/node_modules/playwright,readonly" \
  --mount "type=bind,src=$browser_container_core,dst=/opt/cyanrex-browser/node_modules/playwright-core,readonly" \
  --entrypoint /bin/bash "$browser_container_image" \
  "$browser_container_root/scripts/platform-browser-container-entry.sh" \
  "$browser_container_mode" "$browser_container_root" "$browser_container_session" "$browser_container_body"
