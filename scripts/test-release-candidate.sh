#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERIFIER="$ROOT_DIR/scripts/release-candidate.py"
EVIDENCE_TOOL="$ROOT_DIR/scripts/live-kernel-evidence.py"
WORK_DIR="$(mktemp -d)"
PACKAGE_NAME="cyanrex-lab-1.2.3-20260904-010203"
PACKAGE_DIR="$WORK_DIR/source/$PACKAGE_NAME"
BUNDLE_DIR="$WORK_DIR/bundle"
REVISION="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
trap 'rm -rf "$WORK_DIR"' EXIT

sha256_path() {
  python3 - "$1" <<'PY'
import hashlib
import sys

digest = hashlib.sha256()
with open(sys.argv[1], "rb") as handle:
    while chunk := handle.read(1024 * 1024):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

write_checksum() {
  local target="$1"
  printf '%s  %s\n' "$(sha256_path "$target")" "$(basename "$target")" > "$target.sha256"
}

expect_failure() {
  local label="$1"
  local expected="$2"
  shift 2
  if "$@" > "$WORK_DIR/failure.log" 2>&1; then
    echo "Release candidate verifier test failed: $label was accepted." >&2
    exit 1
  fi
  if ! grep -Fq -- "$expected" "$WORK_DIR/failure.log"; then
    echo "Release candidate verifier test failed: $label returned the wrong error." >&2
    cat "$WORK_DIR/failure.log" >&2
    exit 1
  fi
}

clone_bundle() {
  local name="$1"
  local destination="$WORK_DIR/$name"
  cp -R "$BUNDLE_DIR" "$destination"
  printf '%s\n' "$destination"
}

mkdir -p "$PACKAGE_DIR" "$BUNDLE_DIR"
printf 'services: {}\n' > "$PACKAGE_DIR/docker-compose.yml"
printf 'mock image archive\n' > "$PACKAGE_DIR/cyanrex-images.tar"
IMAGE_HASH="$(sha256_path "$PACKAGE_DIR/cyanrex-images.tar")"
cat > "$PACKAGE_DIR/release-metadata.json" <<EOF
{
  "schemaVersion": 1,
  "package": {
    "name": "$PACKAGE_NAME",
    "version": "1.2.3",
    "createdAt": "2026-09-04T01:02:03Z"
  },
  "source": {
    "revision": "$REVISION",
    "state": "clean",
    "tag": "v1.2.3"
  },
  "compose": {
    "file": "docker-compose.yml",
    "source": "docker/docker-compose.distribution.yml"
  },
  "images": {
    "mode": "built",
    "references": {
      "engine": "cyanrex/cyanrex-engine:1.2.3",
      "frontend": "cyanrex/cyanrex-frontend:1.2.3",
      "postgres": "postgres:16"
    },
    "contentIds": {
      "engine": "sha256:$(printf '%064d' 1)",
      "frontend": "sha256:$(printf '%064d' 2)",
      "postgres": "sha256:$(printf '%064d' 3)"
    },
    "archive": {
      "file": "cyanrex-images.tar",
      "sha256": "$IMAGE_HASH"
    }
  },
  "integrity": {
    "algorithm": "sha256",
    "manifest": "checksums.sha256"
  }
}
EOF
for package_file in docker-compose.yml cyanrex-images.tar release-metadata.json; do
  printf '%s  %s\n' "$(sha256_path "$PACKAGE_DIR/$package_file")" "$package_file"
done > "$PACKAGE_DIR/checksums.sha256"
tar -czf "$BUNDLE_DIR/$PACKAGE_NAME.tar.gz" -C "$WORK_DIR/source" "$PACKAGE_NAME"
write_checksum "$BUNDLE_DIR/$PACKAGE_NAME.tar.gz"

cat > "$WORK_DIR/environment.json" <<'EOF'
{
  "overall_ok": true,
  "generated_at": "2026-09-04T01:02:04Z",
  "runtime_mode": "native-linux",
  "checks": [
    {"name": "kernel", "ok": true, "detail": "Linux 6.8.0 test kernel"}
  ]
}
EOF
cat > "$WORK_DIR/event.json" <<'EOF'
{
  "timestamp": "2026-09-04T01:02:05Z",
  "event_type": "ebpf.kernel_ringbuf",
  "payload": {
    "program_name": "release-kernel-smoke-0123456789abcdef",
    "bytes": 64
  }
}
EOF
python3 "$EVIDENCE_TOOL" create \
  --output "$BUNDLE_DIR/cyanrex-live-kernel-acceptance.json" \
  --environment "$WORK_DIR/environment.json" \
  --event "$WORK_DIR/event.json" \
  --program-name release-kernel-smoke-0123456789abcdef \
  --pin-path /sys/fs/bpf/release-kernel-smoke-0123456789abcdef \
  --release-metadata "$PACKAGE_DIR/release-metadata.json" >/dev/null
write_checksum "$BUNDLE_DIR/cyanrex-live-kernel-acceptance.json"

python3 "$VERIFIER" verify "$BUNDLE_DIR" \
  --expect-version 1.2.3 \
  --expect-revision "$REVISION" \
  --expect-tag v1.2.3 \
  --expect-source-state clean \
  --expect-image-mode built >/dev/null
expect_failure "wrong version expectation" "package version does not match expectation" \
  python3 "$VERIFIER" verify "$BUNDLE_DIR" --expect-version 9.9.9

OUTER_TAMPER="$(clone_bundle outer-tamper)"
printf 'tampered\n' >> "$OUTER_TAMPER/$PACKAGE_NAME.tar.gz"
expect_failure "outer archive tampering" "SHA-256 mismatch" \
  python3 "$VERIFIER" verify "$OUTER_TAMPER"

REPORT_TAMPER="$(clone_bundle report-tamper)"
python3 - "$REPORT_TAMPER/cyanrex-live-kernel-acceptance.json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    report = json.load(handle)
report["candidate"]["releaseMetadataSha256"] = "0" * 64
with open(path, "w", encoding="utf-8") as handle:
    json.dump(report, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
write_checksum "$REPORT_TAMPER/cyanrex-live-kernel-acceptance.json"
expect_failure "candidate evidence mismatch" "does not match the archived release metadata" \
  python3 "$VERIFIER" verify "$REPORT_TAMPER"

INTERNAL_TAMPER="$(clone_bundle internal-tamper)"
mkdir -p "$WORK_DIR/internal-source"
cp -R "$PACKAGE_DIR" "$WORK_DIR/internal-source/$PACKAGE_NAME"
printf 'tampered: true\n' >> "$WORK_DIR/internal-source/$PACKAGE_NAME/docker-compose.yml"
tar -czf "$INTERNAL_TAMPER/$PACKAGE_NAME.tar.gz" -C "$WORK_DIR/internal-source" "$PACKAGE_NAME"
write_checksum "$INTERNAL_TAMPER/$PACKAGE_NAME.tar.gz"
expect_failure "package member tampering" "checksum manifest does not match docker-compose.yml" \
  python3 "$VERIFIER" verify "$INTERNAL_TAMPER"

TRAVERSAL="$(clone_bundle traversal)"
python3 - "$TRAVERSAL/$PACKAGE_NAME.tar.gz" <<'PY'
import io
import sys
import tarfile

with tarfile.open(sys.argv[1], "w:gz") as archive:
    member = tarfile.TarInfo("../escape")
    member.size = 6
    archive.addfile(member, io.BytesIO(b"escape"))
PY
write_checksum "$TRAVERSAL/$PACKAGE_NAME.tar.gz"
expect_failure "path traversal archive" "escapes its package root" \
  python3 "$VERIFIER" verify "$TRAVERSAL"
[ ! -e "$WORK_DIR/escape" ]

SYMLINK="$(clone_bundle symlink)"
python3 - "$SYMLINK/$PACKAGE_NAME.tar.gz" "$PACKAGE_NAME" <<'PY'
import sys
import tarfile

with tarfile.open(sys.argv[1], "w:gz") as archive:
    root = tarfile.TarInfo(sys.argv[2])
    root.type = tarfile.DIRTYPE
    archive.addfile(root)
    member = tarfile.TarInfo(f"{sys.argv[2]}/outside")
    member.type = tarfile.SYMTYPE
    member.linkname = "../../outside"
    archive.addfile(member)
PY
write_checksum "$SYMLINK/$PACKAGE_NAME.tar.gz"
expect_failure "symbolic-link archive" "unsupported member type" \
  python3 "$VERIFIER" verify "$SYMLINK"

DUPLICATE="$(clone_bundle duplicate)"
python3 - "$DUPLICATE/$PACKAGE_NAME.tar.gz" "$PACKAGE_NAME" <<'PY'
import io
import sys
import tarfile

with tarfile.open(sys.argv[1], "w:gz") as archive:
    root = tarfile.TarInfo(sys.argv[2])
    root.type = tarfile.DIRTYPE
    archive.addfile(root)
    for content in (b"first", b"second"):
        member = tarfile.TarInfo(f"{sys.argv[2]}/duplicate")
        member.size = len(content)
        archive.addfile(member, io.BytesIO(content))
PY
write_checksum "$DUPLICATE/$PACKAGE_NAME.tar.gz"
expect_failure "duplicate archive member" "duplicate member" \
  python3 "$VERIFIER" verify "$DUPLICATE"

UNCHECKSUMMED="$(clone_bundle unchecksummed)"
mkdir -p "$WORK_DIR/unchecksummed-source"
cp -R "$PACKAGE_DIR" "$WORK_DIR/unchecksummed-source/$PACKAGE_NAME"
printf 'not covered by the manifest\n' > "$WORK_DIR/unchecksummed-source/$PACKAGE_NAME/rogue.txt"
tar -czf "$UNCHECKSUMMED/$PACKAGE_NAME.tar.gz" \
  -C "$WORK_DIR/unchecksummed-source" "$PACKAGE_NAME"
write_checksum "$UNCHECKSUMMED/$PACKAGE_NAME.tar.gz"
expect_failure "unchecksummed package member" "checksum manifest file set is invalid" \
  python3 "$VERIFIER" verify "$UNCHECKSUMMED"

AMBIGUOUS="$(clone_bundle ambiguous)"
cp "$AMBIGUOUS/$PACKAGE_NAME.tar.gz" \
  "$AMBIGUOUS/cyanrex-lab-9.9.9-20260904-010203.tar.gz"
expect_failure "ambiguous bundle" "exactly one Cyanrex .tar.gz archive" \
  python3 "$VERIFIER" verify "$AMBIGUOUS"

echo "Release candidate bundle verifier checks passed."
