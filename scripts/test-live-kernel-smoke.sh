#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_SCRIPT="$ROOT_DIR/scripts/live-kernel-smoke.sh"
SOURCE_EVIDENCE_TOOL="$ROOT_DIR/scripts/live-kernel-evidence.py"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

bash -n "$SOURCE_SCRIPT"
"$SOURCE_SCRIPT" --help >/dev/null
mkdir -p "$WORK_DIR/bin" "$WORK_DIR/runtime"
cp "$SOURCE_SCRIPT" "$WORK_DIR/runtime/live-kernel-smoke.sh"
cp "$SOURCE_EVIDENCE_TOOL" "$WORK_DIR/runtime/live-kernel-evidence.py"
chmod +x "$WORK_DIR/runtime/live-kernel-smoke.sh" "$WORK_DIR/runtime/live-kernel-evidence.py"
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
assert report["schemaVersion"] == 2
assert report["result"] == "passed"
assert report["candidate"]["package"]["version"] == "1.2.3"
assert report["candidate"]["source"]["tag"] == "v1.2.3"
assert report["candidate"]["releaseMetadataSha256"] == metadata_hash
assert report["environment"]["runtime_mode"] == "docker"
assert report["exercise"]["programName"].startswith("release-kernel-smoke-")
assert report["exercise"]["event"]["bytes"] == 32
assert report["exercise"]["event"]["programName"] == report["exercise"]["programName"]
assert report["cleanup"] == {"exactPinDetached": True, "remainingAttachments": 0}
PY
python3 "$WORK_DIR/runtime/live-kernel-evidence.py" verify "$WORK_DIR/evidence.json" \
  --release-metadata "$WORK_DIR/runtime/release-metadata.json" \
  --expect-version 1.2.3 \
  --expect-revision aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --expect-tag v1.2.3 \
  --expect-source-state clean \
  --expect-image-mode built >/dev/null
python3 - "$WORK_DIR/evidence.json" "$WORK_DIR/tampered-evidence.json" \
  "$WORK_DIR/legacy-evidence.json" "$WORK_DIR/runtime/release-metadata.json" \
  "$WORK_DIR/mismatched-metadata.json" "$WORK_DIR/duplicate-keys.json" \
  "$WORK_DIR/create-environment.json" "$WORK_DIR/create-event.json" <<'PY'
import copy
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    report = json.load(handle)
tampered = copy.deepcopy(report)
tampered["exercise"]["event"]["bytes"] = 0
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    json.dump(tampered, handle)
legacy = copy.deepcopy(report)
legacy["schemaVersion"] = 1
legacy["exercise"]["event"].pop("programName")
with open(sys.argv[3], "w", encoding="utf-8") as handle:
    json.dump(legacy, handle)
with open(sys.argv[4], encoding="utf-8") as handle:
    metadata = json.load(handle)
metadata["images"]["archive"]["sha256"] = "c" * 64
with open(sys.argv[5], "w", encoding="utf-8") as handle:
    json.dump(metadata, handle)
with open(sys.argv[6], "w", encoding="utf-8") as handle:
    handle.write('{"schemaVersion":2,"schemaVersion":2}')
with open(sys.argv[7], "w", encoding="utf-8") as handle:
    json.dump(report["environment"], handle)
with open(sys.argv[8], "w", encoding="utf-8") as handle:
    json.dump({
        "timestamp": report["exercise"]["event"]["timestamp"],
        "event_type": report["exercise"]["event"]["type"],
        "payload": {
            "program_name": report["exercise"]["programName"],
            "bytes": report["exercise"]["event"]["bytes"],
        },
    }, handle)
PY
python3 "$WORK_DIR/runtime/live-kernel-evidence.py" verify "$WORK_DIR/legacy-evidence.json" \
  --release-metadata "$WORK_DIR/runtime/release-metadata.json" >/dev/null
if python3 "$WORK_DIR/runtime/live-kernel-evidence.py" verify \
  "$WORK_DIR/tampered-evidence.json" >"$WORK_DIR/tampered.log" 2>&1; then
  echo "Live kernel smoke tool test failed: tampered evidence was accepted." >&2
  exit 1
fi
if python3 "$WORK_DIR/runtime/live-kernel-evidence.py" verify "$WORK_DIR/evidence.json" \
  --release-metadata "$WORK_DIR/mismatched-metadata.json" >"$WORK_DIR/mismatch.log" 2>&1; then
  echo "Live kernel smoke tool test failed: mismatched candidate metadata was accepted." >&2
  exit 1
fi
if python3 "$WORK_DIR/runtime/live-kernel-evidence.py" verify \
  "$WORK_DIR/duplicate-keys.json" >"$WORK_DIR/duplicate.log" 2>&1; then
  echo "Live kernel smoke tool test failed: duplicate JSON keys were accepted." >&2
  exit 1
fi
if python3 "$WORK_DIR/runtime/live-kernel-evidence.py" create \
  --output "$WORK_DIR/evidence.json" \
  --environment "$WORK_DIR/create-environment.json" \
  --event "$WORK_DIR/create-event.json" \
  --program-name "$(cat "$WORK_DIR/program")" \
  --pin-path /sys/fs/bpf/cyanrex/mock \
  --release-metadata "$WORK_DIR/runtime/release-metadata.json" \
  >"$WORK_DIR/overwrite.log" 2>&1; then
  echo "Live kernel smoke tool test failed: existing evidence was overwritten." >&2
  exit 1
fi
grep -q 'evidence output already exists' "$WORK_DIR/overwrite.log"
if find "$WORK_DIR" -maxdepth 1 -name '.evidence.json.tmp-*' -print -quit | grep -q .; then
  echo "Live kernel smoke tool test failed: atomic-write temporary file remains." >&2
  exit 1
fi
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
