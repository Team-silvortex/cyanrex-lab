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
if [ -f "$ROOT_DIR/scripts/live-kernel-evidence.py" ]; then
  EVIDENCE_TOOL="$ROOT_DIR/scripts/live-kernel-evidence.py"
else
  EVIDENCE_TOOL="$ROOT_DIR/live-kernel-evidence.py"
fi

usage() {
  cat <<'EOF'
Usage: ./scripts/live-kernel-smoke.sh

Runs destructive, privileged release acceptance against an already-running disposable Cyanrex stack:
  1. authenticate and require an empty administrator attachment set;
  2. load and attach the built-in sched_switch ringbuf template with the Aya backend;
  3. require a real kernel ringbuf event to reach the authenticated event history;
  4. detach the exact program and verify no tracked attachment remains.

Environment:
  CYANREX_SMOKE_ENGINE_URL=http://127.0.0.1:8080  Engine endpoint
  CYANREX_SMOKE_ORIGIN=http://localhost:3000       Trusted request origin
  CYANREX_KERNEL_SMOKE_STREAM_SECONDS=6            Kernel sampling window (2-30)
  CYANREX_KERNEL_SMOKE_POLL_ATTEMPTS=80            Event polling attempts (1-200)
  CYANREX_KERNEL_SMOKE_POLL_INTERVAL=0.25          Seconds between polls
  CYANREX_KERNEL_SMOKE_REPORT=                      Optional machine-readable evidence output path

The runtime environment file must provide CYANREX_ADMIN_PASSWORD and
CYANREX_ADMIN_TOTP_SECRET. Run only on a disposable privileged Linux acceptance host.
EOF
}

if [[ "${1:-}" =~ ^(-h|--help|help)$ ]]; then
  usage
  exit 0
fi
if [ "$#" -ne 0 ]; then
  echo "Error: live kernel smoke does not accept positional arguments." >&2
  usage
  exit 1
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required for live kernel acceptance." >&2
    exit 1
  fi
}

for command in curl python3; do require_cmd "$command"; done
if [ ! -f "$ENV_FILE" ]; then
  echo "Error: runtime configuration is missing: $ENV_FILE" >&2
  exit 1
fi
set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a
: "${CYANREX_ADMIN_PASSWORD:?CYANREX_ADMIN_PASSWORD is required}"
: "${CYANREX_ADMIN_TOTP_SECRET:?CYANREX_ADMIN_TOTP_SECRET is required}"

ENGINE_URL="${CYANREX_SMOKE_ENGINE_URL:-http://127.0.0.1:${CYANREX_ENGINE_PORT:-8080}}"
ORIGIN="${CYANREX_SMOKE_ORIGIN:-http://localhost:${CYANREX_FRONTEND_PORT:-3000}}"
STREAM_SECONDS="${CYANREX_KERNEL_SMOKE_STREAM_SECONDS:-6}"
POLL_ATTEMPTS="${CYANREX_KERNEL_SMOKE_POLL_ATTEMPTS:-80}"
POLL_INTERVAL="${CYANREX_KERNEL_SMOKE_POLL_INTERVAL:-0.25}"
REPORT_PATH="${CYANREX_KERNEL_SMOKE_REPORT:-}"
PROGRAM_NAME="$(python3 -c 'import uuid; print("release-kernel-smoke-" + uuid.uuid4().hex[:16])')"
if [[ ! "$STREAM_SECONDS" =~ ^[0-9]+$ ]] || (( STREAM_SECONDS < 2 || STREAM_SECONDS > 30 )); then
  echo "Error: CYANREX_KERNEL_SMOKE_STREAM_SECONDS must be an integer from 2 to 30." >&2
  exit 1
fi
if [[ ! "$POLL_ATTEMPTS" =~ ^[0-9]+$ ]] || (( POLL_ATTEMPTS < 1 || POLL_ATTEMPTS > 200 )); then
  echo "Error: CYANREX_KERNEL_SMOKE_POLL_ATTEMPTS must be an integer from 1 to 200." >&2
  exit 1
fi
if [[ ! "$POLL_INTERVAL" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "Error: CYANREX_KERNEL_SMOKE_POLL_INTERVAL must be a non-negative number." >&2
  exit 1
fi
if [ -n "$REPORT_PATH" ] && [ -e "$REPORT_PATH" ]; then
  echo "Error: live kernel evidence output already exists: $REPORT_PATH" >&2
  exit 1
fi
if [ -n "$REPORT_PATH" ] && [ ! -f "$EVIDENCE_TOOL" ]; then
  echo "Error: live kernel evidence tool is missing: $EVIDENCE_TOOL" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d)"
COOKIE_JAR="$WORK_DIR/cookies.txt"
LOGIN_JSON="$WORK_DIR/login.json"
TEMPLATES_JSON="$WORK_DIR/templates.json"
ENVIRONMENT_JSON="$WORK_DIR/environment.json"
RUN_JSON="$WORK_DIR/run.json"
RESPONSE_JSON="$WORK_DIR/response.json"
DETACH_JSON="$WORK_DIR/detach.json"
MATCHED_EVENT_JSON="$WORK_DIR/matched-event.json"
PIN_PATH=""
RUN_ATTEMPTED=0

cleanup() {
  local exit_code="${1:-$?}"
  trap - EXIT INT TERM
  if [ "$RUN_ATTEMPTED" -eq 1 ] && [ -f "$COOKIE_JAR" ]; then
    PIN_PATH="$PIN_PATH" DETACH_JSON="$DETACH_JSON" python3 - <<'PY' || true
import json
import os
with open(os.environ["DETACH_JSON"], "w", encoding="utf-8") as handle:
    json.dump({"pin_path": os.environ.get("PIN_PATH") or None}, handle)
PY
    curl -sS -b "$COOKIE_JAR" -H "Origin: $ORIGIN" -H 'Content-Type: application/json' \
      --data-binary "@$DETACH_JSON" "$ENGINE_URL/ebpf/detach" >/dev/null 2>&1 || true
  fi
  rm -rf "$WORK_DIR"
  exit "$exit_code"
}
trap 'cleanup $?' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

export LOGIN_JSON
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
PY

echo "[cyanrex] Authenticating privileged live-kernel acceptance..."
curl -fsS -c "$COOKIE_JAR" -H 'Content-Type: application/json' \
  --data-binary "@$LOGIN_JSON" "$ENGINE_URL/auth/login" > "$RESPONSE_JSON"

curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/helper/environment" > "$ENVIRONMENT_JSON"
python3 - "$ENVIRONMENT_JSON" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
checks = payload.get("checks")
kernel = next((item for item in checks or [] if item.get("name") == "kernel"), None)
if not payload.get("runtime_mode") or not kernel or not kernel.get("detail"):
    raise SystemExit("live kernel environment report omits runtime mode or kernel release")
PY

curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/ebpf/attachments" > "$RESPONSE_JSON"
python3 - "$RESPONSE_JSON" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if payload.get("pin_paths"):
    raise SystemExit("live kernel acceptance requires an empty administrator attachment set")
PY

curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/ebpf/templates" > "$TEMPLATES_JSON"
export TEMPLATES_JSON RUN_JSON STREAM_SECONDS PROGRAM_NAME
python3 - <<'PY'
import json
import os

with open(os.environ["TEMPLATES_JSON"], encoding="utf-8") as handle:
    templates = json.load(handle)
template_id = "ringbuf-hi-freq-sampler"
template = next((item for item in templates if item.get("id") == template_id), None)
if not template or not template.get("code"):
    raise SystemExit(f"required live kernel template is missing: {template_id}")
with open(os.environ["RUN_JSON"], "w", encoding="utf-8") as handle:
    json.dump({
        "code": template["code"],
        "template_id": template_id,
        "program_name": os.environ["PROGRAM_NAME"],
        "runtime_backend": "aya",
        "sampling_per_sec": 20,
        "stream_seconds": int(os.environ["STREAM_SECONDS"]),
        "enable_kernel_stream": True,
    }, handle)
PY

echo "[cyanrex] Loading and attaching the release kernel smoke program..."
RUN_ATTEMPTED=1
curl -fsS -b "$COOKIE_JAR" -H "Origin: $ORIGIN" -H 'Content-Type: application/json' \
  --data-binary "@$RUN_JSON" "$ENGINE_URL/ebpf/run" > "$RESPONSE_JSON"
PIN_PATH="$(python3 - "$RESPONSE_JSON" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if not payload.get("success") or payload.get("stage") != "run" or not payload.get("pin_path"):
    raise SystemExit(f"live kernel run failed: {payload.get('message', 'unknown error')}")
print(payload["pin_path"])
PY
)"

curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/ebpf/attachments" > "$RESPONSE_JSON"
python3 - "$RESPONSE_JSON" "$PIN_PATH" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if sys.argv[2] not in payload.get("pin_paths", []):
    raise SystemExit("live kernel attachment is not tracked after a successful run")
PY

echo "[cyanrex] Waiting for a real kernel ringbuf event..."
event_ready=0
for ((attempt = 1; attempt <= POLL_ATTEMPTS; attempt++)); do
  curl -fsS -b "$COOKIE_JAR" \
    "$ENGINE_URL/events?category=kernel&since_minutes=5&limit=500" > "$RESPONSE_JSON"
  if python3 - "$RESPONSE_JSON" "$MATCHED_EVENT_JSON" <<'PY'
import json
import os
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    events = json.load(handle)
matched = next(
    (
        event for event in events
        if event.get("event_type") == "ebpf.kernel_ringbuf"
        and event.get("payload", {}).get("program_name") == os.environ["PROGRAM_NAME"]
        and event.get("payload", {}).get("bytes", 0) > 0
    ),
    None,
)
if matched:
    with open(sys.argv[2], "w", encoding="utf-8") as handle:
        json.dump(matched, handle)
raise SystemExit(0 if matched is not None else 1)
PY
  then
    event_ready=1
    break
  fi
  sleep "$POLL_INTERVAL"
done
if [ "$event_ready" -ne 1 ]; then
  echo "Error: no live kernel ringbuf event reached the authenticated event history." >&2
  python3 -m json.tool "$RESPONSE_JSON" >&2 || true
  exit 1
fi

PIN_PATH="$PIN_PATH" DETACH_JSON="$DETACH_JSON" python3 - <<'PY'
import json
import os
with open(os.environ["DETACH_JSON"], "w", encoding="utf-8") as handle:
    json.dump({"pin_path": os.environ["PIN_PATH"]}, handle)
PY
curl -fsS -b "$COOKIE_JAR" -H "Origin: $ORIGIN" -H 'Content-Type: application/json' \
  --data-binary "@$DETACH_JSON" "$ENGINE_URL/ebpf/detach" > "$RESPONSE_JSON"
python3 - "$RESPONSE_JSON" "$PIN_PATH" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if not payload.get("ok") or not payload.get("clean") or sys.argv[2] not in payload.get("detached", []):
    raise SystemExit("live kernel detach did not report a clean exact-path removal")
PY
RUN_ATTEMPTED=0

curl -fsS -b "$COOKIE_JAR" "$ENGINE_URL/ebpf/attachments" > "$RESPONSE_JSON"
python3 - "$RESPONSE_JSON" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
if payload.get("pin_paths"):
    raise SystemExit("tracked live kernel attachments remain after cleanup")
PY

if [ -n "$REPORT_PATH" ]; then
  metadata_arguments=()
  if [ -f "$ROOT_DIR/release-metadata.json" ]; then
    metadata_arguments=(--release-metadata "$ROOT_DIR/release-metadata.json")
  fi
  python3 "$EVIDENCE_TOOL" create \
    --output "$REPORT_PATH" \
    --environment "$ENVIRONMENT_JSON" \
    --event "$MATCHED_EVENT_JSON" \
    --program-name "$PROGRAM_NAME" \
    --pin-path "$PIN_PATH" \
    "${metadata_arguments[@]}"
fi

echo "[cyanrex] Live kernel attach, event stream, and detach acceptance passed."
