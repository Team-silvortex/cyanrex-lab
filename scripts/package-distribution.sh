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
COMPOSE_TEMPLATE="$ROOT_DIR/docker/docker-compose.distribution.yml"
CHECKSUM_CMD=()

print_help() {
  cat <<'EOF'
Usage: ./scripts/package-distribution.sh [--version <version>] [--engine-image <image>] [--frontend-image <image>] [--compose-template <path>] [--output <dir>] [--skip-checks] [--skip-build] [--no-compress]

Build and package distributable artifacts for offline or air-gapped classroom deployment:
  - Build and export engine/frontend Docker images.
  - Create a reusable distribution directory with compose templates and docs.
  - Export a tarball containing image layers and deployment scripts.

Options:
  --version <version>   Version tag for package image names (default from engine/Cargo.toml)
  --engine-image <image>    Fully qualified engine image (for skip-build or custom naming)
  --frontend-image <image>  Fully qualified frontend image (for skip-build or custom naming)
  --compose-template <path> Compose template file (default: docker/docker-compose.distribution.yml)
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
    --compose-template)
      if [[ $# -lt 2 ]]; then
        echo "Error: --compose-template requires a value." >&2
        exit 1
      fi
      COMPOSE_TEMPLATE="$2"
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
resolve_checksum_command() {
  if [ "${#CHECKSUM_CMD[@]}" -gt 0 ]; then
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    CHECKSUM_CMD=(sha256sum)
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    CHECKSUM_CMD=(shasum -a 256)
    return
  fi

  echo "Error: neither sha256sum nor shasum is available." >&2
  exit 1
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
normalize_image_name() {
  local value="$1"
  if [[ "$value" != *:* && "$value" != *"@"* ]]; then
    echo "${value}:${VERSION}"
    return
  fi
  echo "$value"
}
resolve_images() {
  if [[ -z "$ENGINE_IMAGE" ]]; then
    ENGINE_IMAGE="cyanrex/cyanrex-engine:${VERSION}"
  fi
  if [[ -z "$FRONTEND_IMAGE" ]]; then
    FRONTEND_IMAGE="cyanrex/cyanrex-frontend:${VERSION}"
  fi
  ENGINE_IMAGE="$(normalize_image_name "$ENGINE_IMAGE")"
  FRONTEND_IMAGE="$(normalize_image_name "$FRONTEND_IMAGE")"
}
validate_compose_template() {
  if [[ ! -f "$COMPOSE_TEMPLATE" ]]; then
    echo "Error: compose template not found: $COMPOSE_TEMPLATE" >&2
    exit 1
  fi
}
build_artifacts() {
  local engine_image="$1"
  local frontend_image="$2"

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

generate_deploy_script() {
  local package_dir="$1"
  local deploy_script="$package_dir/deploy.sh"
  local run_script="$package_dir/run.sh"
  local stop_script="$package_dir/stop.sh"

cat > "$deploy_script" <<EOF
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="\${SCRIPT_DIR}/docker-compose.yml"
ENV_FILE="\${SCRIPT_DIR}/.env"
IMAGE_ARCHIVE="\${SCRIPT_DIR}/cyanrex-images.tar"
COMPOSE_CMD=()

ENGINE_IMAGE_DEFAULT="${ENGINE_IMAGE}"
FRONTEND_IMAGE_DEFAULT="${FRONTEND_IMAGE}"
IMAGE_TAG="${VERSION}"

require_cmd() {
  if ! command -v "\$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required but not installed." >&2
    exit 1
  fi
}

validate_required_secrets() {
  local missing=0
  local key
  local value
  local defaults

  defaults=(
    "POSTGRES_PASSWORD:replace-with-a-long-random-password"
    "CYANREX_ADMIN_PASSWORD:replace-with-a-long-random-password"
    "CYANREX_ADMIN_TOTP_SECRET:REPLACEWITHBASE32SECRET"
  )

  for key in "\${defaults[@]}"; do
    IFS=":" read -r key_name invalid_value <<<"\$key"
    value="\${!key_name:-}"
    if [ -z "\${value}" ]; then
      echo "Error: \${key_name} is required in .env." >&2
      missing=1
      continue
    fi
    if [ "\${value}" = "\$invalid_value" ]; then
      echo "Error: \${key_name} still contains a placeholder in .env." >&2
      missing=1
    fi
  done

  if [ "\$missing" -ne 0 ]; then
    echo "Hint: copy .env.example -> .env and replace placeholders with secure values." >&2
    exit 1
  fi
}

resolve_compose_command() {
  if [ "${#COMPOSE_CMD[@]}" -gt 0 ]; then
    return
  fi

  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
    return
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
    return
  fi

  echo "Error: neither 'docker compose' nor legacy 'docker-compose' is available." >&2
  exit 1
}

compose() {
  resolve_compose_command
  "${COMPOSE_CMD[@]}" -f "\$COMPOSE_FILE" "\$@"
}

compose_has_project() {
  resolve_compose_command
  "${COMPOSE_CMD[@]}" -f "\$COMPOSE_FILE" config --services >/dev/null 2>&1
}

load_images_if_missing() {
  if [[ ! -f "\$IMAGE_ARCHIVE" ]]; then
    echo "[cyanrex] Image archive not found: \$IMAGE_ARCHIVE"
    return
  fi

  if docker image inspect "\$CYANREX_ENGINE_IMAGE" >/dev/null 2>&1 && docker image inspect "\$CYANREX_FRONTEND_IMAGE" >/dev/null 2>&1; then
    echo "[cyanrex] Engine and frontend images already present locally; skipping docker load."
    return
  fi

  echo "[cyanrex] Loading packaged images..."
  docker load -i "\$IMAGE_ARCHIVE"
}

load_env_file() {
  if [ -f "\$ENV_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    source "\$ENV_FILE"
    set +a
  fi
}

check_http_ok() {
  local url="\$1"
  if command -v wget >/dev/null 2>&1; then
    wget -qO- "\$url" >/dev/null 2>&1
    return
  fi
  if command -v curl >/dev/null 2>&1; then
    curl -fsS "\$url" >/dev/null 2>&1
    return
  fi
  return 1
}

wait_for_health() {
  local timeout_sec="\${CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS:-90}"
  local sleep_sec="\${CYANREX_DEPLOY_HEALTH_POLL_SECONDS:-2}"
  local health_host="\${CYANREX_BIND_ADDRESS:-127.0.0.1}"
  local endpoint
  if [ "\$health_host" = "0.0.0.0" ]; then
    health_host="127.0.0.1"
  fi
  endpoint="http://\${health_host}:\${CYANREX_ENGINE_PORT:-8080}/health"
  local elapsed=0

  echo "[cyanrex] Waiting for engine health endpoint: \$endpoint"
  while true; do
    if check_http_ok "\$endpoint"; then
      echo "[cyanrex] Engine health check passed."
      return
    fi

    if [ "\$elapsed" -ge "\$timeout_sec" ]; then
      echo "[cyanrex] Timeout waiting for engine health at \$endpoint." >&2
      exit 1
    fi

    sleep "\$sleep_sec"
    elapsed=\$((elapsed + sleep_sec))
  done
}

print_endpoints() {
  echo "[cyanrex] Service endpoints:"
  echo "  frontend: http://\${CYANREX_BIND_ADDRESS:-127.0.0.1}:\${CYANREX_FRONTEND_PORT:-3000}"
  echo "  engine:   http://\${CYANREX_BIND_ADDRESS:-127.0.0.1}:\${CYANREX_ENGINE_PORT:-8080}/health"
  echo "  postgres: \${CYANREX_BIND_ADDRESS:-127.0.0.1}:\${CYANREX_POSTGRES_PORT:-15432}"
  echo "  login:    admin (check .env credentials)"
}

check_runtime() {
  require_cmd docker
  if ! docker info >/dev/null 2>&1; then
    echo "Error: docker daemon is not running or not reachable." >&2
    exit 1
  fi
  if ! compose_has_project; then
    echo "Error: invalid compose file: \$COMPOSE_FILE" >&2
    exit 1
  fi
}

deploy_up() {
  if [ ! -f "\$ENV_FILE" ]; then
    echo "Error: .env is required for deploy." >&2
    echo "Run: cp .env.example .env && edit secrets before deploy." >&2
    exit 1
  fi

  load_env_file

  CYANREX_IMAGE_TAG="\${CYANREX_IMAGE_TAG:-\$IMAGE_TAG}"
  CYANREX_ENGINE_IMAGE="\${CYANREX_ENGINE_IMAGE:-\$ENGINE_IMAGE_DEFAULT}"
  CYANREX_FRONTEND_IMAGE="\${CYANREX_FRONTEND_IMAGE:-\$FRONTEND_IMAGE_DEFAULT}"

  export CYANREX_IMAGE_TAG CYANREX_ENGINE_IMAGE CYANREX_FRONTEND_IMAGE

  validate_required_secrets
  check_runtime
  load_images_if_missing

  compose up -d "\$@"

  if [ "\${CYANREX_DEPLOY_WAIT_FOR_HEALTH:-1}" != "0" ]; then
    wait_for_health
  fi
  print_endpoints
}

deploy_down() {
  compose down "\$@"
}

deploy_status() {
  compose ps
}

deploy_logs() {
  compose logs -f "\$@"
}

usage() {
  cat <<'USAGE'
Usage: ./deploy.sh [up|down|status|logs] [service...]

Commands:
  up [services...]     Start the stack (default if omitted)
  down                 Stop the stack
  status               Show compose status
  logs [service]       Tail logs (default: all services)

Environment:
  CYANREX_DEPLOY_WAIT_FOR_HEALTH=1        Wait for engine /health after start (default: 1)
  CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS=90 Health wait timeout
  CYANREX_DEPLOY_HEALTH_POLL_SECONDS=2    Health poll interval
USAGE
}

ACTION="\${1:-up}"
shift || true

case "\$ACTION" in
  up|start)
    deploy_up "\$@"
    ;;
  down|stop)
    load_env_file
    check_runtime
    deploy_down "\$@"
    ;;
  status)
    load_env_file
    check_runtime
    deploy_status
    ;;
  logs)
    load_env_file
    check_runtime
    deploy_logs "\$@"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Error: unknown command '\$ACTION'." >&2
    usage
    exit 1
    ;;
esac
EOF
chmod +x "$deploy_script"

cat > "$run_script" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/deploy.sh" "${1:-up}" "${@:2}"
EOF
chmod +x "$run_script"

cat > "$stop_script" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/deploy.sh" down "$@"
EOF
chmod +x "$stop_script"
}

generate_package_readme() {
  local package_dir="$1"
  local readme_file="$package_dir/README-DEPLOY.md"

cat > "$readme_file" <<EOF
Release package: ${PACKAGE_NAME}
Version: ${VERSION}
Built at (UTC): ${PACKAGE_TAG}

Package contents:
- docker-compose.yml
- .env.example
- deploy.sh
- run.sh (wrapper for deploy.sh up)
- stop.sh (wrapper for deploy.sh down)
- cyanrex-images.tar (engine + frontend docker images)
- LICENSE and quick-readme docs

Usage:
1) Copy or move this directory to your target machine.
2) Copy .env.example -> .env and edit secrets immediately:
   cp .env.example .env
3) Start:
   ./run.sh
   # same as ./deploy.sh up
4) Status:
   ./deploy.sh status
5) Logs:
   ./deploy.sh logs
6) Stop:
   ./stop.sh
   # same as ./deploy.sh down

The package defaults to local-loopback publishing. Edit .env before exposing ports.
Set CYANREX_DEPLOY_WAIT_FOR_HEALTH=0 to skip startup health checks.
EOF
}

generate_checksums() {
  local package_dir="$1"
  resolve_checksum_command
  (cd "$package_dir" && "${CHECKSUM_CMD[@]}" docker-compose.yml .env.example LICENSE README.md README-en.md README-zh-CN.md README-docker.md README-DEPLOY.md manifest.env deploy.sh run.sh stop.sh cyanrex-images.tar > checksums.sha256)
}

resolve_version

require_cmd tar
resolve_checksum_command
require_cmd docker
validate_compose_template

if ! docker info >/dev/null 2>&1; then
  echo "Error: docker daemon is not running or not reachable." >&2
  exit 1
fi

if [[ "$SKIP_CHECKS" == "0" ]]; then
  "$ROOT_DIR/scripts/quality-gate.sh" --format-only
fi

resolve_images
validate_compose_template
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

cp "$COMPOSE_TEMPLATE" "$PACKAGE_DIR/docker-compose.yml"
cp "$ROOT_DIR/docker/.env.example" "$PACKAGE_DIR/.env.example"
cp "$ROOT_DIR/LICENSE" "$PACKAGE_DIR/LICENSE"
cp "$ROOT_DIR/README.md" "$PACKAGE_DIR/README.md"
cp "$ROOT_DIR/docs/en/README.md" "$PACKAGE_DIR/README-en.md"
cp "$ROOT_DIR/docs/zh-CN/README.md" "$PACKAGE_DIR/README-zh-CN.md"
cp "$ROOT_DIR/docker/README.md" "$PACKAGE_DIR/README-docker.md"

cat > "$PACKAGE_DIR/manifest.env" <<EOF
PACKAGE_NAME="${PACKAGE_NAME}"
PACKAGE_VERSION="${VERSION}"
PACKAGE_TIMESTAMP_UTC="${PACKAGE_TAG}"
ENGINE_IMAGE="${ENGINE_IMAGE}"
FRONTEND_IMAGE="${FRONTEND_IMAGE}"
COMPOSE_TEMPLATE="${COMPOSE_TEMPLATE}"
EOF

echo "[cyanrex] Exporting Docker images..."
export_images "$ENGINE_IMAGE" "$FRONTEND_IMAGE" "$IMAGE_ARCHIVE"
echo "[cyanrex] Exported image archive: $IMAGE_ARCHIVE"

generate_deploy_script "$PACKAGE_DIR"
generate_package_readme "$PACKAGE_DIR"
generate_checksums "$PACKAGE_DIR"

if [[ "$NO_COMPRESS" == "1" ]]; then
  echo "[cyanrex] Output package directory: $PACKAGE_DIR"
  echo "[cyanrex] You can optionally archive manually:"
  echo "  tar -czf ${ARCHIVE_PATH} -C $(printf '%q' "$OUTPUT_DIR") $(printf '%q' "$PACKAGE_NAME")"
else
  echo "[cyanrex] Creating archive..."
  tar -czf "$ARCHIVE_PATH" -C "$OUTPUT_DIR" "$PACKAGE_NAME"
  "${CHECKSUM_CMD[@]}" "$ARCHIVE_PATH" > "${ARCHIVE_PATH}.sha256"
  rm -rf "$PACKAGE_DIR"
  echo "[cyanrex] Package created:"
  echo "  $ARCHIVE_PATH"
  echo "  ${ARCHIVE_PATH}.sha256"
fi
