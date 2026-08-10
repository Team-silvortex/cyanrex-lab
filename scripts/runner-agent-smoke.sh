#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/../docker/.env" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
  ROOT_DIR="$SCRIPT_DIR"
fi
if [ -f "$ROOT_DIR/docker/.env" ]; then
  ENV_FILE="$ROOT_DIR/docker/.env"
else
  ENV_FILE="$ROOT_DIR/.env"
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required." >&2
    exit 1
  fi
}

if [ ! -f "$ENV_FILE" ]; then
  echo "Error: runtime configuration is missing: $ENV_FILE" >&2
  exit 1
fi
require_cmd curl
require_cmd python3

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a
: "${CYANREX_ADMIN_PASSWORD:?CYANREX_ADMIN_PASSWORD is required}"
: "${CYANREX_ADMIN_TOTP_SECRET:?CYANREX_ADMIN_TOTP_SECRET is required}"

ENGINE_URL="${CYANREX_SMOKE_ENGINE_URL:-http://127.0.0.1:${CYANREX_ENGINE_PORT:-8080}}"
ORIGIN="${CYANREX_SMOKE_ORIGIN:-http://localhost:${CYANREX_FRONTEND_PORT:-3000}}"
AGENT_ID="${1:-${CYANREX_AGENT_ID:-cyanrex-docker-compiler}}"
CYANREX_AGENT_ID="$AGENT_ID"
WORK_DIR="$(mktemp -d)"
COOKIE_JAR="$WORK_DIR/cookies.txt"
LOGIN_JSON="$WORK_DIR/login.json"
SUBMIT_JSON="$WORK_DIR/submit.json"
RESPONSE_JSON="$WORK_DIR/response.json"
JOB_ID=""
cleanup() {
  if [ -n "$JOB_ID" ] && [ -f "$COOKIE_JAR" ]; then
    curl -sS -b "$COOKIE_JAR" \
      -H "Origin: $ORIGIN" \
      -H 'Content-Type: application/json' \
      --data-binary "{\"job_id\":\"$JOB_ID\"}" \
      "$ENGINE_URL/ebpf/check/remote/cancel" >/dev/null 2>&1 || true
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

export CYANREX_AGENT_ID LOGIN_JSON SUBMIT_JSON
python3 - <<'PY'
import base64
import hashlib
import hmac
import json
import os
import struct
import time

secret = os.environ["CYANREX_ADMIN_TOTP_SECRET"].strip().replace(" ", "").upper()
secret += "=" * ((8 - len(secret) % 8) % 8)
key = base64.b32decode(secret)
counter = int(time.time()) // 30
digest = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
offset = digest[-1] & 0x0F
number = struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF
otp = f"{number % 1_000_000:06d}"
with open(os.environ["LOGIN_JSON"], "w", encoding="utf-8") as handle:
    json.dump({"username": "admin", "password": os.environ["CYANREX_ADMIN_PASSWORD"], "otp": otp}, handle)
with open(os.environ["SUBMIT_JSON"], "w", encoding="utf-8") as handle:
    json.dump({
        "agent_id": os.environ.get("CYANREX_AGENT_ID", "cyanrex-docker-compiler"),
        "program_name": "agent-smoke",
        "code": "int cyanrex_agent_smoke(void) { return 0; }\n",
    }, handle)
PY

echo "[cyanrex] Logging in for the user-scoped remote-check smoke test..."
curl -fsS -c "$COOKIE_JAR" \
  -H 'Content-Type: application/json' \
  --data-binary "@$LOGIN_JSON" \
  "$ENGINE_URL/auth/login" > "$RESPONSE_JSON"

echo "[cyanrex] Waiting for compile Agent '$AGENT_ID'..."
agent_ready=0
for _ in $(seq 1 30); do
  curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/ebpf/check/backends" > "$RESPONSE_JSON"
  if python3 - "$RESPONSE_JSON" "$AGENT_ID" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
raise SystemExit(0 if any(item.get("agent_id") == sys.argv[2] for item in payload.get("agents", [])) else 1)
PY
  then
    agent_ready=1
    break
  fi
  sleep 1
done
if [ "$agent_ready" -ne 1 ]; then
  echo "Error: compile Agent '$AGENT_ID' did not become available." >&2
  exit 1
fi

curl -fsS -b "$COOKIE_JAR" \
  -H "Origin: $ORIGIN" \
  -H 'Content-Type: application/json' \
  --data-binary "@$SUBMIT_JSON" \
  "$ENGINE_URL/ebpf/check/remote" > "$RESPONSE_JSON"
JOB_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["job_id"])' "$RESPONSE_JSON")"

echo "[cyanrex] Waiting for remote compile job $JOB_ID..."
for _ in $(seq 1 70); do
  curl -fsS -b "$COOKIE_JAR" \
    "$ENGINE_URL/ebpf/check/remote?job_id=$JOB_ID" > "$RESPONSE_JSON"
  state="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["state"])' "$RESPONSE_JSON")"
  case "$state" in
    succeeded)
      python3 - "$RESPONSE_JSON" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if not (payload.get("result") or {}).get("ok"):
    raise SystemExit("remote compiler returned a non-successful result")
PY
      echo "[cyanrex] Remote compiler smoke test passed."
      exit 0
      ;;
    failed|cancelled|expired)
      echo "Error: remote compile job ended as '$state'." >&2
      python3 -m json.tool "$RESPONSE_JSON" >&2
      exit 1
      ;;
  esac
  sleep 0.5
done

echo "Error: remote compile job timed out." >&2
exit 1
