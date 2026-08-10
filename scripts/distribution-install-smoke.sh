#!/usr/bin/env bash
set -euo pipefail

PACKAGE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$PACKAGE_DIR/.env"
WORK_DIR=""
STACK_ATTEMPTED=0

usage() {
  cat <<'EOF'
Usage: ./install-smoke.sh

Runs a destructive installation acceptance test inside a freshly extracted Cyanrex package:
  1. verify packaged checksums and helper syntax;
  2. create disposable secrets and start the complete Compose stack;
  3. verify Engine, frontend, PostgreSQL dependency readiness, and CSP;
  4. start the optional compiler Agent and run authenticated remote diagnostics;
  5. stop the stack and remove its disposable volume/configuration.

Environment:
  CYANREX_SMOKE_BIND_ADDRESS=127.0.0.1 Loopback address used for published ports
  CYANREX_SMOKE_ENGINE_PORT=8080       Host Engine port
  CYANREX_SMOKE_FRONTEND_PORT=3000     Host frontend port
  CYANREX_SMOKE_POSTGRES_PORT=15432    Host PostgreSQL port
  CYANREX_SMOKE_SKIP_AGENT=0           Set to 1 to skip Runner Agent validation
  CYANREX_SMOKE_KEEP=0                 Set to 1 to keep stack and generated .env for debugging
EOF
}

if [[ "${1:-}" =~ ^(-h|--help|help)$ ]]; then
  usage
  exit 0
fi
if [ "$#" -ne 0 ]; then
  echo "Error: install smoke does not accept positional arguments." >&2
  usage
  exit 1
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required for distribution smoke testing." >&2
    exit 1
  fi
}

cleanup() {
  local exit_code="${1:-$?}"
  trap - EXIT INT TERM
  if [ "${CYANREX_SMOKE_KEEP:-0}" = "1" ]; then
    echo "[cyanrex] Keeping smoke stack and configuration for inspection."
    exit "$exit_code"
  fi
  if [ "$STACK_ATTEMPTED" -eq 1 ] && [ -f "$ENV_FILE" ]; then
    "$PACKAGE_DIR/deploy.sh" down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  rm -f "$ENV_FILE" "$PACKAGE_DIR/.runner-agent-token"
  if [ -n "$WORK_DIR" ]; then rm -rf "$WORK_DIR"; fi
  exit "$exit_code"
}
trap 'cleanup $?' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

resolve_checksum_command() {
  if command -v sha256sum >/dev/null 2>&1; then
    CHECKSUM_CMD=(sha256sum -c)
  elif command -v shasum >/dev/null 2>&1; then
    CHECKSUM_CMD=(shasum -a 256 -c)
  else
    echo "Error: neither sha256sum nor shasum is available." >&2
    exit 1
  fi
}

replace_env_value() {
  local key="$1"
  local value="$2"
  local temporary
  temporary="$(mktemp "${ENV_FILE}.tmp.XXXXXX")"
  awk -v key="$key" -v value="$value" '
    BEGIN { replaced = 0 }
    index($0, key "=") == 1 {
      if (!replaced) print key "=" value
      replaced = 1
      next
    }
    { print }
    END { if (!replaced) print key "=" value }
  ' "$ENV_FILE" > "$temporary"
  chmod 600 "$temporary"
  mv -f "$temporary" "$ENV_FILE"
}

for command in docker curl python3 openssl awk mktemp; do require_cmd "$command"; done
for file in checksums.sha256 manifest.env docker-compose.yml .env.example \
  deploy.sh runner-agent.sh runner-agent-smoke.sh cyanrex-images.tar; do
  if [ ! -f "$PACKAGE_DIR/$file" ]; then
    echo "Error: distribution package is missing '$file'." >&2
    exit 1
  fi
done
if [ -e "$ENV_FILE" ]; then
  echo "Error: refusing to overwrite existing runtime configuration: $ENV_FILE" >&2
  echo "Run this smoke test only in a freshly extracted disposable package." >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "Error: Docker daemon is unavailable." >&2
  exit 1
fi

resolve_checksum_command
echo "[cyanrex] Verifying packaged file checksums..."
(cd "$PACKAGE_DIR" && "${CHECKSUM_CMD[@]}" checksums.sha256 >/dev/null)
bash -n "$PACKAGE_DIR/deploy.sh" "$PACKAGE_DIR/run.sh" "$PACKAGE_DIR/stop.sh" \
  "$PACKAGE_DIR/runner-agent.sh" "$PACKAGE_DIR/runner-agent-smoke.sh"

ENGINE_PORT="${CYANREX_SMOKE_ENGINE_PORT:-8080}"
FRONTEND_PORT="${CYANREX_SMOKE_FRONTEND_PORT:-3000}"
POSTGRES_PORT="${CYANREX_SMOKE_POSTGRES_PORT:-15432}"
BIND_ADDRESS="${CYANREX_SMOKE_BIND_ADDRESS:-127.0.0.1}"
if [[ ! "$BIND_ADDRESS" =~ ^127\. ]]; then
  echo "Error: distribution smoke must bind within the IPv4 loopback range." >&2
  exit 1
fi
SMOKE_ID="dist-smoke-$$"
cp "$PACKAGE_DIR/.env.example" "$ENV_FILE"
chmod 600 "$ENV_FILE"
replace_env_value POSTGRES_PASSWORD "$(openssl rand -hex 24)"
replace_env_value CYANREX_ADMIN_PASSWORD "$(openssl rand -hex 24)"
replace_env_value CYANREX_ADMIN_TOTP_SECRET AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
replace_env_value CYANREX_BIND_ADDRESS "$BIND_ADDRESS"
replace_env_value CYANREX_INSTANCE_ID "$SMOKE_ID"
replace_env_value COMPOSE_PROJECT_NAME "cyanrex-$SMOKE_ID"
replace_env_value CYANREX_ENGINE_PORT "$ENGINE_PORT"
replace_env_value CYANREX_FRONTEND_PORT "$FRONTEND_PORT"
replace_env_value CYANREX_POSTGRES_PORT "$POSTGRES_PORT"
replace_env_value CYANREX_POSTGRES_VOLUME_NAME "cyanrex-postgres-$SMOKE_ID"

WORK_DIR="$(mktemp -d)"
STACK_ATTEMPTED=1
echo "[cyanrex] Starting extracted distribution package..."
CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS=120 "$PACKAGE_DIR/deploy.sh" up --pull never

ENGINE_URL="http://$BIND_ADDRESS:$ENGINE_PORT"
FRONTEND_URL="http://$BIND_ADDRESS:$FRONTEND_PORT"
curl -fsS "$ENGINE_URL/health" > "$WORK_DIR/health.json"
python3 - "$WORK_DIR/health.json" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if payload.get("status") != "ok":
    raise SystemExit("Engine health payload is not ok")
PY
frontend_ready=0
for ((attempt = 1; attempt <= 30; attempt++)); do
  if curl -fsS -D "$WORK_DIR/frontend.headers" "$FRONTEND_URL/login" \
    > "$WORK_DIR/frontend.html" 2>/dev/null; then
    frontend_ready=1
    break
  fi
  sleep 1
done
if [ "$frontend_ready" -ne 1 ]; then
  echo "Error: frontend did not become ready at $FRONTEND_URL/login." >&2
  exit 1
fi
grep -q 'CYANREX' "$WORK_DIR/frontend.html"
tr -d '\r' < "$WORK_DIR/frontend.headers" | grep -Eiq \
  "^content-security-policy:.*http://localhost:${ENGINE_PORT}([ ;]|$)"
"$PACKAGE_DIR/deploy.sh" status

if [ "${CYANREX_SMOKE_SKIP_AGENT:-0}" != "1" ]; then
  "$PACKAGE_DIR/runner-agent.sh" start --agent-id "$SMOKE_ID-agent"
  CYANREX_SMOKE_ENGINE_URL="$ENGINE_URL" \
    CYANREX_SMOKE_ORIGIN="http://localhost:$FRONTEND_PORT" \
    "$PACKAGE_DIR/runner-agent-smoke.sh" "$SMOKE_ID-agent"
fi

echo "[cyanrex] Distribution installation smoke test passed."
