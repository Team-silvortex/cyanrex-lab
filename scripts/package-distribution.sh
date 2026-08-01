#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/dist"
PACKAGE_TAG="$(date -u +%Y%m%d-%H%M%S)"
VERSION=""
ENGINE_IMAGE=""
FRONTEND_IMAGE=""
SKIP_CHECKS=0
SKIP_BUILD=0
NO_COMPRESS=0

print_help() {
  cat <<'EOF'
Usage: ./scripts/package-distribution.sh [--version <version>] [--engine-image <image>] [--frontend-image <image>] [--output <dir>] [--skip-checks] [--skip-build] [--no-compress]

Build and package distributable artifacts for offline or air-gapped classroom deployment:
  - Build and export engine/frontend Docker images.
  - Create a reusable distribution directory with compose templates and docs.
  - Export a tarball containing image layers and startup instructions.

Options:
  --version <version>   Version tag for package image names (default from engine/Cargo.toml)
  --engine-image <image>    Fully qualified engine image (for skip-build or custom naming)
  --frontend-image <image>  Fully qualified frontend image (for skip-build or custom naming)
  --output <dir>        Output directory (default: ./dist)
  --skip-checks          Skip quality-gate checks before packaging
  --skip-build           Skip Docker rebuild (use prebuilt images already available locally)
  --no-compress          Keep package directory instead of creating tar.gz archive
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      if [[ $# -lt 2 ]]; then
        echo "Error: --version requires a value." >&2
        exit 1
      fi
      VERSION="$2"
      shift 2
      ;;
    --engine-image)
      if [[ $# -lt 2 ]]; then
        echo "Error: --engine-image requires a value." >&2
        exit 1
      fi
      ENGINE_IMAGE="$2"
      shift 2
      ;;
    --frontend-image)
      if [[ $# -lt 2 ]]; then
        echo "Error: --frontend-image requires a value." >&2
        exit 1
      fi
      FRONTEND_IMAGE="$2"
      shift 2
      ;;
    --output)
      if [[ $# -lt 2 ]]; then
        echo "Error: --output requires a value." >&2
        exit 1
      fi
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --skip-checks)
      SKIP_CHECKS=1
      shift
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --no-compress)
      NO_COMPRESS=1
      shift
      ;;
    -h|--help)
      print_help
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      print_help
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required but not installed." >&2
    exit 1
  fi
}

resolve_version() {
  if [[ -n "$VERSION" ]]; then
    echo "$VERSION"
    return
  fi

  if [[ ! -f "$ROOT_DIR/engine/Cargo.toml" ]]; then
    echo "Error: cannot find engine/Cargo.toml." >&2
    exit 1
  fi

  local detected
  detected="$(awk -F'=' '/^version = / {gsub(/"/, "", $2); gsub(/ /, "", $2); print $2; exit}' "$ROOT_DIR/engine/Cargo.toml")"
  if [[ -z "$detected" ]]; then
    echo "Error: failed to parse version from engine/Cargo.toml." >&2
    exit 1
  fi
  VERSION="$detected"
}

resolve_images() {
  if [[ -z "$ENGINE_IMAGE" ]]; then
    ENGINE_IMAGE="cyanrex/cyanrex-engine:${VERSION}"
  fi
  if [[ -z "$FRONTEND_IMAGE" ]]; then
    FRONTEND_IMAGE="cyanrex/cyanrex-frontend:${VERSION}"
  fi
}

build_artifacts() {
  local engine_image="$1"
  local frontend_image="$2"

  echo "[cyanrex] Building release engine binary..."
  cargo build --manifest-path "$ROOT_DIR/engine/Cargo.toml" --release

  echo "[cyanrex] Building frontend dependencies and production assets..."
  npm ci --prefix "$ROOT_DIR/frontend"
  npm --prefix "$ROOT_DIR/frontend" run build

  echo "[cyanrex] Building Docker images for distribution..."
  docker build -t "$engine_image" -f "$ROOT_DIR/engine/Dockerfile" "$ROOT_DIR/engine"
  docker build -t "$frontend_image" -f "$ROOT_DIR/frontend/Dockerfile" "$ROOT_DIR/frontend"
}

assert_images() {
  local engine_image="$1"
  local frontend_image="$2"

  if ! docker image inspect "$engine_image" >/dev/null 2>&1; then
    echo "Error: engine image '${engine_image}' is missing locally." >&2
    echo "Hint: build it first or pass --engine-image with an existing image." >&2
    exit 1
  fi
  if ! docker image inspect "$frontend_image" >/dev/null 2>&1; then
    echo "Error: frontend image '${frontend_image}' is missing locally." >&2
    echo "Hint: build it first or pass --frontend-image with an existing image." >&2
    exit 1
  fi
}

export_images() {
  local engine_image="$1"
  local frontend_image="$2"
  local output_image_archive="$3"

  docker save "$engine_image" "$frontend_image" -o "$output_image_archive"
}

resolve_version

require_cmd tar
require_cmd sha256sum
require_cmd docker
if [[ "$SKIP_BUILD" == "0" ]]; then
  require_cmd cargo
  require_cmd npm
fi

if ! docker info >/dev/null 2>&1; then
  echo "Error: docker daemon is not running or not reachable." >&2
  exit 1
fi

if [[ "$SKIP_CHECKS" == "0" ]]; then
  "$ROOT_DIR/scripts/quality-gate.sh" --format-only
fi

resolve_images
PACKAGE_NAME="cyanrex-lab-${VERSION}-${PACKAGE_TAG}"
PACKAGE_DIR="$OUTPUT_DIR/$PACKAGE_NAME"
ARCHIVE_PATH="$OUTPUT_DIR/${PACKAGE_NAME}.tar.gz"
IMAGE_ARCHIVE="${PACKAGE_DIR}/cyanrex-images.tar"

mkdir -p "$OUTPUT_DIR"

if [[ -d "$PACKAGE_DIR" ]]; then
  rm -rf "$PACKAGE_DIR"
fi
mkdir -p "$PACKAGE_DIR"

if [[ "$SKIP_BUILD" == "0" ]]; then
  build_artifacts "$ENGINE_IMAGE" "$FRONTEND_IMAGE"
fi

assert_images "$ENGINE_IMAGE" "$FRONTEND_IMAGE"

cp "$ROOT_DIR/docker/docker-compose.distribution.yml" "$PACKAGE_DIR/docker-compose.yml"
cp "$ROOT_DIR/docker/.env.example" "$PACKAGE_DIR/.env.example"
cp "$ROOT_DIR/LICENSE" "$PACKAGE_DIR/LICENSE"
cp "$ROOT_DIR/README.md" "$PACKAGE_DIR/README.md"
cp "$ROOT_DIR/docs/en/README.md" "$PACKAGE_DIR/README-en.md"
cp "$ROOT_DIR/docs/zh-CN/README.md" "$PACKAGE_DIR/README-zh-CN.md"

cat > "$PACKAGE_DIR/run.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
IMAGE_ARCHIVE="\${SCRIPT_DIR}/cyanrex-images.tar"
IMAGE_TAG="${VERSION}"
ENGINE_IMAGE_DEFAULT="${ENGINE_IMAGE}"
FRONTEND_IMAGE_DEFAULT="${FRONTEND_IMAGE}"
cd "\$SCRIPT_DIR"

if [[ -f "\$IMAGE_ARCHIVE" ]]; then
  docker load -i "\$IMAGE_ARCHIVE"
fi

export CYANREX_IMAGE_TAG="\${CYANREX_IMAGE_TAG:-\$IMAGE_TAG}"
export CYANREX_ENGINE_IMAGE="\${CYANREX_ENGINE_IMAGE:-\$ENGINE_IMAGE_DEFAULT}"
export CYANREX_FRONTEND_IMAGE="\${CYANREX_FRONTEND_IMAGE:-\$FRONTEND_IMAGE_DEFAULT}"

echo "[cyanrex] Starting services with image tag: \$CYANREX_IMAGE_TAG"
docker compose -f "\$SCRIPT_DIR/docker-compose.yml" up -d
EOF
chmod +x "$PACKAGE_DIR/run.sh"

cat > "$PACKAGE_DIR/stop.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
docker compose -f "$SCRIPT_DIR/docker-compose.yml" down
EOF
chmod +x "$PACKAGE_DIR/stop.sh"

cat > "$PACKAGE_DIR/README-DEPLOY.md" <<EOF
Release package: ${PACKAGE_NAME}
Version: ${VERSION}
Built at (UTC): ${PACKAGE_TAG}

Package contents:
- docker-compose.yml
- .env.example
- run.sh
- stop.sh
- cyanrex-images.tar (engine + frontend docker images)
- LICENSE and quick-readme docs

Usage:
1) Copy or move this directory to your target machine.
2) Copy .env.example -> .env and edit secrets if needed:
   cp .env.example .env
3) (Optional) Use a custom image tag:
   export CYANREX_IMAGE_TAG=${VERSION}
4) (Optional) Use custom image names:
   export CYANREX_ENGINE_IMAGE=${ENGINE_IMAGE}
   export CYANREX_FRONTEND_IMAGE=${FRONTEND_IMAGE}
5) Start:
   ./run.sh
6) Stop:
   ./stop.sh

The package defaults to local-loopback publishing. Edit .env before going online.
EOF

echo "[cyanrex] Exporting Docker images..."
export_images "$ENGINE_IMAGE" "$FRONTEND_IMAGE" "$IMAGE_ARCHIVE"

if [[ "$NO_COMPRESS" == "1" ]]; then
  echo "[cyanrex] Output package directory: $PACKAGE_DIR"
  echo "[cyanrex] You can optionally archive manually:"
  echo "  tar -czf ${ARCHIVE_PATH} -C $(printf '%q' "$OUTPUT_DIR") $(printf '%q' "$PACKAGE_NAME")"
else
  echo "[cyanrex] Creating archive..."
  tar -czf "$ARCHIVE_PATH" -C "$OUTPUT_DIR" "$PACKAGE_NAME"
  sha256sum "$ARCHIVE_PATH" > "${ARCHIVE_PATH}.sha256"
  rm -rf "$PACKAGE_DIR"
  echo "[cyanrex] Package created:"
  echo "  $ARCHIVE_PATH"
  echo "  ${ARCHIVE_PATH}.sha256"
fi
