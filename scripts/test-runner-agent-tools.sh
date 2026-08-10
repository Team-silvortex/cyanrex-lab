#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

cp "$ROOT_DIR/scripts/runner-agent.sh" "$WORK_DIR/runner-agent.sh"
cp "$ROOT_DIR/scripts/runner-agent-smoke.sh" "$WORK_DIR/runner-agent-smoke.sh"
cp "$ROOT_DIR/docker/docker-compose.distribution.yml" "$WORK_DIR/docker-compose.yml"
printf '%s\n' \
  'POSTGRES_PASSWORD=test-postgres-password' \
  'CYANREX_ADMIN_PASSWORD=test-admin-password' \
  'CYANREX_ADMIN_TOTP_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' \
  'CYANREX_RUNNER_AGENT_TOKEN=' > "$WORK_DIR/.env"
chmod 600 "$WORK_DIR/.env"

bash -n "$WORK_DIR/runner-agent.sh" "$WORK_DIR/runner-agent-smoke.sh"
output="$(bash "$WORK_DIR/runner-agent.sh" prepare --agent-id fixture-compiler)"
token="$(awk -F= '/^CYANREX_RUNNER_AGENT_TOKEN=/ {print $2}' "$WORK_DIR/.env")"

if [ "${#token}" -ne 64 ]; then
  echo "Runner Agent tool test failed: generated token length is invalid." >&2
  exit 1
fi
if ! grep -qx 'CYANREX_AGENT_ID=fixture-compiler' "$WORK_DIR/.env"; then
  echo "Runner Agent tool test failed: explicit Agent ID was not persisted." >&2
  exit 1
fi
if ! grep -qx "CYANREX_AGENT_RUNTIME_UID=$(id -u)" "$WORK_DIR/.env" ||
  ! grep -qx "CYANREX_AGENT_RUNTIME_GID=$(id -g)" "$WORK_DIR/.env"; then
  echo "Runner Agent tool test failed: token owner UID/GID was not persisted." >&2
  exit 1
fi
if [ "$(<"$WORK_DIR/.runner-agent-token")" != "$token" ]; then
  echo "Runner Agent tool test failed: Docker Secret does not match runtime configuration." >&2
  exit 1
fi
if [[ "$output" == *"$token"* ]]; then
  echo "Runner Agent tool test failed: manager printed the bootstrap token." >&2
  exit 1
fi

if stat -f '%Lp' "$WORK_DIR/.runner-agent-token" >/dev/null 2>&1; then
  mode="$(stat -f '%Lp' "$WORK_DIR/.runner-agent-token")"
else
  mode="$(stat -c '%a' "$WORK_DIR/.runner-agent-token")"
fi
if [ "$mode" != "600" ]; then
  echo "Runner Agent tool test failed: Docker Secret mode is $mode instead of 600." >&2
  exit 1
fi

bash "$WORK_DIR/runner-agent.sh" prepare --agent-id fixture-compiler >/dev/null
if [ "$(awk -F= '/^CYANREX_RUNNER_AGENT_TOKEN=/ {print $2}' "$WORK_DIR/.env")" != "$token" ]; then
  echo "Runner Agent tool test failed: repeated prepare rotated a valid token." >&2
  exit 1
fi
if bash "$WORK_DIR/runner-agent.sh" prepare --agent-id '../unsafe' >/dev/null 2>&1; then
  echo "Runner Agent tool test failed: unsafe Agent ID was accepted." >&2
  exit 1
fi

echo "Runner Agent management tool checks passed."
