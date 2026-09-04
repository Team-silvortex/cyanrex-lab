#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_SCRIPT="$ROOT_DIR/scripts/package-distribution.sh"
SMOKE_SCRIPT="$ROOT_DIR/scripts/distribution-install-smoke.sh"
METADATA_SCRIPT="$ROOT_DIR/scripts/release-metadata.mjs"
CANDIDATE_SCRIPT="$ROOT_DIR/scripts/release-candidate.py"
EVIDENCE_SCRIPT="$ROOT_DIR/scripts/live-kernel-evidence.py"
RELEASE_WORKFLOW="$ROOT_DIR/.github/workflows/release-validation.yml"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

bash -n "$PACKAGE_SCRIPT" "$SMOKE_SCRIPT"
"$SMOKE_SCRIPT" --help >/dev/null
python3 "$CANDIDATE_SCRIPT" --help >/dev/null

assert_contains() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq -- "$pattern" "$file"; then
    echo "Distribution tool test failed: '$pattern' missing from $file." >&2
    exit 1
  fi
}

assert_contains "$PACKAGE_SCRIPT" 'distribution-install-smoke.sh'
assert_contains "$PACKAGE_SCRIPT" 'live-kernel-smoke.sh'
assert_contains "$PACKAGE_SCRIPT" 'live-kernel-evidence.py'
assert_contains "$PACKAGE_SCRIPT" 'release-metadata.json'
assert_contains "$RELEASE_WORKFLOW" 'needs: metadata'
assert_contains "$RELEASE_WORKFLOW" 'release-metadata.mjs" --verify'
assert_contains "$RELEASE_WORKFLOW" '--expect-source-state clean --expect-image-mode built'
assert_contains "$RELEASE_WORKFLOW" 'CYANREX_SMOKE_RUN_LIVE_KERNEL: "1"'
assert_contains "$RELEASE_WORKFLOW" 'CYANREX_KERNEL_SMOKE_REPORT:'
assert_contains "$RELEASE_WORKFLOW" 'cyanrex-live-kernel-acceptance.json.sha256'
assert_contains "$RELEASE_WORKFLOW" 'release-candidate.py" verify'
assert_contains "$RELEASE_WORKFLOW" 'cp "$report" "$RUNNER_TEMP/cyanrex-release/"'
assert_contains "$RELEASE_WORKFLOW" '--expect-revision "$RELEASE_REVISION"'
assert_contains "$RELEASE_WORKFLOW" '--expect-source-state clean'
assert_contains "$RELEASE_WORKFLOW" '"$package_dir/install-smoke.sh"'
assert_contains "$RELEASE_WORKFLOW" 'actions/upload-artifact@v4'
if grep -Eq 'contents:[[:space:]]*write|gh release|action-gh-release' "$RELEASE_WORKFLOW"; then
  echo "Distribution tool test failed: release validation must not publish releases." >&2
  exit 1
fi
assert_contains "$PACKAGE_SCRIPT" 'install-smoke.sh'
assert_contains "$PACKAGE_SCRIPT" 'POSTGRES_IMAGE="${POSTGRES_IMAGE:-postgres:16}"'
assert_contains "$PACKAGE_SCRIPT" 'docker save "$engine_image" "$frontend_image" "$postgres_image"'
assert_contains "$PACKAGE_SCRIPT" 'ENGINE_RUST_IMAGE:-rust:bookworm'
assert_contains "$PACKAGE_SCRIPT" '"$ROOT_DIR/engine/Dockerfile"'
assert_contains "$PACKAGE_SCRIPT" '"$ROOT_DIR"'
assert_contains "$ROOT_DIR/engine/Dockerfile" 'COPY modules ./modules'
assert_contains "$ROOT_DIR/engine/Dockerfile" 'CYANREX_MODULES_DIR=/app/modules'
assert_contains "$PACKAGE_SCRIPT" 'FRONTEND_NPM_REGISTRY:-https://registry.npmjs.org'
assert_contains "$PACKAGE_SCRIPT" 'basename "$ARCHIVE_PATH"'
assert_contains "$PACKAGE_SCRIPT" "printf 'POSTGRES_IMAGE=%q"
assert_contains "$PACKAGE_SCRIPT" 'compose --profile runner-agent down'
assert_contains "$SMOKE_SCRIPT" 'up --pull never'
assert_contains "$SMOKE_SCRIPT" 'metadata["images"]["contentIds"]'
assert_contains "$SMOKE_SCRIPT" 'docker", "image", "inspect", "--format", "{{.Id}}"'
assert_contains "$SMOKE_SCRIPT" 'CYANREX_ENGINE_IMAGE="$ENGINE_IMAGE"'
assert_contains "$SMOKE_SCRIPT" 'CYANREX_SMOKE_BIND_ADDRESS'
assert_contains "$SMOKE_SCRIPT" 'frontend_ready'
assert_contains "$SMOKE_SCRIPT" '"$PACKAGE_DIR/live-kernel-smoke.sh"'
assert_contains "$ROOT_DIR/scripts/runner-agent.sh" 'export CYANREX_ENGINE_IMAGE CYANREX_IMAGE_TAG POSTGRES_IMAGE'
for compose_file in docker/docker-compose.yml docker/docker-compose.distribution.yml; do
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_ALLOW_MISSING_ORIGIN:'
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_MODULES_DIR: /app/modules'
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED:'
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT:'
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT:'
  assert_contains "$ROOT_DIR/$compose_file" 'CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS:'
done

mkdir -p "$WORK_DIR/bin" "$WORK_DIR/output" "$WORK_DIR/extracted"
cat > "$WORK_DIR/bin/docker" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}:${2:-}" in
  info:) exit 0 ;;
  image:inspect)
    if [[ "$*" == *"--format"* ]]; then
      case "${*: -1}" in
        cyanrex/cyanrex-engine:*) digit=1 ;;
        cyanrex/cyanrex-frontend:*) digit=2 ;;
        postgres:*) digit=3 ;;
        *) exit 2 ;;
      esac
      printf 'sha256:%064d\n' "$digit"
    fi
    exit 0
    ;;
  save:*)
    output=""
    shift
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "-o" ]; then output="$2"; shift 2; else shift; fi
    done
    [ -n "$output" ] || exit 2
    printf 'mock offline image archive\n' > "$output"
    ;;
  *) echo "Unexpected mock docker call: $*" >&2; exit 2 ;;
esac
MOCK
chmod +x "$WORK_DIR/bin/docker"
PATH="$WORK_DIR/bin:$PATH" "$PACKAGE_SCRIPT" --version 0.2.0 --skip-checks --skip-build \
  --output "$WORK_DIR/output" >/dev/null

archive_checksum="$(find "$WORK_DIR/output" -maxdepth 1 -type f -name '*.tar.gz.sha256' -print -quit)"
[ -n "$archive_checksum" ]
checksum_target="$(awk '{print $2}' "$archive_checksum")"
if [[ "$checksum_target" == */* ]]; then
  echo "Distribution tool test failed: outer checksum contains a build-host path." >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$WORK_DIR/output" && sha256sum -c "$(basename "$archive_checksum")" >/dev/null)
else
  (cd "$WORK_DIR/output" && shasum -a 256 -c "$(basename "$archive_checksum")" >/dev/null)
fi
archive_path="${archive_checksum%.sha256}"
tar -xzf "$archive_path" -C "$WORK_DIR/extracted"
package_dir="$(find "$WORK_DIR/extracted" -mindepth 1 -maxdepth 1 -type d -print -quit)"
[ -x "$package_dir/install-smoke.sh" ]
[ -x "$package_dir/live-kernel-smoke.sh" ]
[ -x "$package_dir/live-kernel-evidence.py" ]
assert_contains "$package_dir/manifest.env" 'POSTGRES_IMAGE=postgres:16'
assert_contains "$package_dir/manifest.env" 'COMPOSE_TEMPLATE=docker/docker-compose.distribution.yml'
node "$METADATA_SCRIPT" --verify "$package_dir" >/dev/null
node - "$package_dir/release-metadata.json" <<'NODE'
const metadata = JSON.parse(require("node:fs").readFileSync(process.argv[2], "utf8"));
for (const [name, digit] of Object.entries({ engine: "1", frontend: "2", postgres: "3" })) {
  const expected = `sha256:${"0".repeat(63)}${digit}`;
  if (metadata.images.contentIds[name] !== expected) {
    throw new Error(`unexpected ${name} image content ID`);
  }
}
NODE
if grep -Fq "$ROOT_DIR" "$package_dir/release-metadata.json" "$package_dir/manifest.env"; then
  echo "Distribution tool test failed: package metadata contains the build-host path." >&2
  exit 1
fi
bash -n "$package_dir/deploy.sh" "$package_dir/run.sh" "$package_dir/stop.sh"
"$package_dir/deploy.sh" --help >/dev/null
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$package_dir" && sha256sum -c checksums.sha256 >/dev/null)
else
  (cd "$package_dir" && shasum -a 256 -c checksums.sha256 >/dev/null)
fi

cat > "$WORK_DIR/environment.json" <<'JSON'
{
  "overall_ok": true,
  "generated_at": "2026-09-04T01:02:04Z",
  "runtime_mode": "native-linux",
  "checks": [{"name": "kernel", "ok": true, "detail": "mock test kernel"}]
}
JSON
cat > "$WORK_DIR/event.json" <<'JSON'
{
  "timestamp": "2026-09-04T01:02:05Z",
  "event_type": "ebpf.kernel_ringbuf",
  "payload": {
    "program_name": "release-kernel-smoke-0123456789abcdef",
    "bytes": 64
  }
}
JSON
report="$WORK_DIR/output/cyanrex-live-kernel-acceptance.json"
python3 "$EVIDENCE_SCRIPT" create --output "$report" \
  --environment "$WORK_DIR/environment.json" --event "$WORK_DIR/event.json" \
  --program-name release-kernel-smoke-0123456789abcdef \
  --pin-path /sys/fs/bpf/release-kernel-smoke-0123456789abcdef \
  --release-metadata "$package_dir/release-metadata.json" >/dev/null
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$WORK_DIR/output" && sha256sum "$(basename "$report")" > "$(basename "$report").sha256")
else
  (cd "$WORK_DIR/output" && shasum -a 256 "$(basename "$report")" > "$(basename "$report").sha256")
fi
python3 "$CANDIDATE_SCRIPT" verify "$WORK_DIR/output" \
  --expect-version 0.2.0 --expect-image-mode prebuilt >/dev/null

echo "Distribution management tool checks passed."
