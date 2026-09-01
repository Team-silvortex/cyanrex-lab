#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_SCRIPT="$ROOT_DIR/scripts/live-kernel-smoke.sh"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

bash -n "$SOURCE_SCRIPT"
"$SOURCE_SCRIPT" --help >/dev/null
mkdir -p "$WORK_DIR/bin" "$WORK_DIR/runtime"
cp "$SOURCE_SCRIPT" "$WORK_DIR/runtime/live-kernel-smoke.sh"
chmod +x "$WORK_DIR/runtime/live-kernel-smoke.sh"
cat > "$WORK_DIR/runtime/.env" <<'EOF'
CYANREX_ADMIN_PASSWORD=test-password
CYANREX_ADMIN_TOTP_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
EOF
cat > "$WORK_DIR/runtime/release-metadata.json" <<'EOF'
{
  "schemaVersion": 1,
  "package": {"name": "cyanrex-lab-1.2.3", "version": "1.2.3", "createdAt": "2026-09-01T00:00:00Z"},
  "source": {"revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "state": "clean", "tag": "v1.2.3"},
  "images": {
    "mode": "built",
    "references": {"engine": "engine:1.2.3", "frontend": "frontend:1.2.3", "postgres": "postgres:16"},
    "contentIds": {
      "engine": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      "frontend": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      "postgres": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    },
    "archive": {"sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
  }
}
EOF

cat > "$WORK_DIR/bin/curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
cookie_file=""
data_file=""
arguments=("$@")
for ((index = 0; index < ${#arguments[@]}; index++)); do
  if [ "${arguments[$index]}" = "-c" ]; then
    cookie_file="${arguments[$((index + 1))]}"
  fi
  if [ "${arguments[$index]}" = "--data-binary" ]; then
    data_file="${arguments[$((index + 1))]#@}"
  fi
done
if [ -n "$cookie_file" ]; then printf 'mock cookie\n' > "$cookie_file"; fi
url="${arguments[-1]}"
state="$(cat "$MOCK_KERNEL_STATE" 2>/dev/null || printf 'empty')"
case "$url" in
  */auth/login)
    printf '{"ok":true}\n'
    ;;
  */helper/environment)
    printf '{"overall_ok":true,"generated_at":"2026-09-01T00:00:00Z","runtime_mode":"docker","runtime_guidance":"mock","checks":[{"name":"kernel","ok":true,"detail":"6.8.0-mock"}]}\n'
    ;;
  */ebpf/templates)
    printf '[{"id":"ringbuf-hi-freq-sampler","code":"SEC(\\"tracepoint/sched/sched_switch\\")"}]\n'
    ;;
  */ebpf/run)
    python3 - "$data_file" "$MOCK_KERNEL_PROGRAM" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    request = json.load(handle)
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    handle.write(request["program_name"])
PY
    printf 'attached' > "$MOCK_KERNEL_STATE"
    printf '{"success":true,"stage":"run","pin_path":"/sys/fs/bpf/cyanrex/mock"}\n'
    ;;
  */ebpf/attachments)
    if [ "$state" = "attached" ]; then
      printf '{"pin_paths":["/sys/fs/bpf/cyanrex/mock"]}\n'
    else
      printf '{"pin_paths":[]}\n'
    fi
    ;;
  */events\?*)
    if [ "${MOCK_KERNEL_MODE:-success}" != "empty-stream" ]; then
      program_name="$(cat "$MOCK_KERNEL_PROGRAM")"
      if [ "${MOCK_KERNEL_MODE:-success}" = "stale-event" ]; then
        program_name="release-kernel-smoke-stale"
      fi
      printf '[{"timestamp":"2026-09-01T00:00:01Z","event_type":"ebpf.kernel_ringbuf","payload":{"program_name":"%s","bytes":32}}]\n' \
        "$program_name"
    else
      printf '[]\n'
    fi
    ;;
  */ebpf/detach)
    printf 'detached' > "$MOCK_KERNEL_STATE"
    printf '{"ok":true,"clean":true,"detached":["/sys/fs/bpf/cyanrex/mock"]}\n'
    ;;
  *)
    echo "Unexpected mock curl URL: $url" >&2
    exit 2
    ;;
esac
MOCK
chmod +x "$WORK_DIR/bin/curl"

run_smoke() {
  local mode="$1"
  local report_path="${2:-}"
  printf 'empty' > "$WORK_DIR/state"
  PATH="$WORK_DIR/bin:$PATH" \
    MOCK_KERNEL_STATE="$WORK_DIR/state" \
    MOCK_KERNEL_PROGRAM="$WORK_DIR/program" \
    MOCK_KERNEL_MODE="$mode" \
    CYANREX_KERNEL_SMOKE_REPORT="$report_path" \
    CYANREX_KERNEL_SMOKE_POLL_ATTEMPTS=2 \
    CYANREX_KERNEL_SMOKE_POLL_INTERVAL=0.01 \
    "$WORK_DIR/runtime/live-kernel-smoke.sh"
}

run_smoke success "$WORK_DIR/evidence.json" >/dev/null
if [ "$(cat "$WORK_DIR/state")" != "detached" ]; then
  echo "Live kernel smoke tool test failed: successful run did not detach." >&2
  exit 1
fi
python3 - "$WORK_DIR/evidence.json" "$WORK_DIR/runtime/release-metadata.json" <<'PY'
import hashlib
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
with open(sys.argv[2], "rb") as handle:
    metadata_hash = hashlib.sha256(handle.read()).hexdigest()
assert report["schemaVersion"] == 1
assert report["result"] == "passed"
assert report["candidate"]["package"]["version"] == "1.2.3"
assert report["candidate"]["source"]["tag"] == "v1.2.3"
assert report["candidate"]["releaseMetadataSha256"] == metadata_hash
assert report["environment"]["runtime_mode"] == "docker"
assert report["exercise"]["programName"].startswith("release-kernel-smoke-")
assert report["exercise"]["event"]["bytes"] == 32
assert report["cleanup"] == {"exactPinDetached": True, "remainingAttachments": 0}
PY
if run_smoke stale-event >"$WORK_DIR/stale.log" 2>&1; then
  echo "Live kernel smoke tool test failed: a stale event was accepted." >&2
  exit 1
fi
if [ "$(cat "$WORK_DIR/state")" != "detached" ]; then
  echo "Live kernel smoke tool test failed: stale-event rejection did not clean up." >&2
  exit 1
fi
if run_smoke empty-stream >"$WORK_DIR/failure.log" 2>&1; then
  echo "Live kernel smoke tool test failed: empty stream was accepted." >&2
  exit 1
fi
if [ "$(cat "$WORK_DIR/state")" != "detached" ]; then
  echo "Live kernel smoke tool test failed: failed run did not clean up." >&2
  exit 1
fi

echo "Live kernel smoke tool checks passed."
