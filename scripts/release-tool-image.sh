#!/usr/bin/env bash

RELEASE_TOOL_CONTAINER=""

cleanup_release_tool_container() {
  if [ -n "$RELEASE_TOOL_CONTAINER" ]; then
    docker rm -f "$RELEASE_TOOL_CONTAINER" >/dev/null 2>&1 || true
    RELEASE_TOOL_CONTAINER=""
  fi
}
trap cleanup_release_tool_container EXIT

export_release_tool() {
  local engine_image="$1"
  local output="$2"
  if ! RELEASE_TOOL_CONTAINER="$(docker create "$engine_image")"; then
    echo "Error: cannot inspect Engine image for the native release tool: $engine_image" >&2
    exit 1
  fi
  if ! docker cp "$RELEASE_TOOL_CONTAINER:/usr/local/bin/cyanrex-release" "$output"; then
    echo "Error: Engine image does not contain /usr/local/bin/cyanrex-release." >&2
    exit 1
  fi
  if ! docker rm "$RELEASE_TOOL_CONTAINER" >/dev/null; then
    echo "Error: cannot remove temporary Engine image container." >&2
    exit 1
  fi
  RELEASE_TOOL_CONTAINER=""
  chmod 755 "$output"
}
