
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.yml"
ENV_FILE="${SCRIPT_DIR}/.env"
IMAGE_ARCHIVE="${SCRIPT_DIR}/cyanrex-images.tar"
COMPOSE_CMD=()


require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
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

  for key in "${defaults[@]}"; do
    IFS=":" read -r key_name invalid_value <<<"$key"
    value="${!key_name:-}"
    if [ -z "${value}" ]; then
      echo "Error: ${key_name} is required in .env." >&2
      missing=1
      continue
    fi
    if [ "${value}" = "$invalid_value" ]; then
      echo "Error: ${key_name} still contains a placeholder in .env." >&2
      missing=1
    fi
  done

  if [ "$missing" -ne 0 ]; then
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
  "${COMPOSE_CMD[@]}" -f "$COMPOSE_FILE" "$@"
}

compose_has_project() {
  resolve_compose_command
  "${COMPOSE_CMD[@]}" -f "$COMPOSE_FILE" config --services >/dev/null 2>&1
}

load_images_if_missing() {
  if [[ ! -f "$IMAGE_ARCHIVE" ]]; then
    echo "[cyanrex] Image archive not found: $IMAGE_ARCHIVE"
    return
  fi

  if docker image inspect "$CYANREX_ENGINE_IMAGE" >/dev/null 2>&1 && docker image inspect "$CYANREX_FRONTEND_IMAGE" >/dev/null 2>&1 && docker image inspect "$POSTGRES_IMAGE" >/dev/null 2>&1; then
    echo "[cyanrex] All packaged images already present locally; skipping docker load."
    return
  fi

  echo "[cyanrex] Loading packaged images..."
  docker load -i "$IMAGE_ARCHIVE"
}

load_env_file() {
  if [ -f "$ENV_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
  fi
}

prepare_health_check() {
  case "${CYANREX_DEPLOY_WAIT_FOR_HEALTH:-1}" in
    0) HEALTH_ARGS=(); return ;;
    1) ;;
    *) echo "Error: CYANREX_DEPLOY_WAIT_FOR_HEALTH must be 0 or 1." >&2; exit 1 ;;
  esac
  local health_host="${CYANREX_BIND_ADDRESS:-127.0.0.1}"
  local health_port="${CYANREX_ENGINE_PORT:-8080}"
  if ! [[ "$health_port" =~ ^0*([1-9][0-9]{0,4})$ ]] || [ "${BASH_REMATCH[1]}" -gt 65535 ]; then
    echo "Error: CYANREX_ENGINE_PORT must be in range 1..65535." >&2
    exit 1
  fi
  if [ "$health_host" = "0.0.0.0" ]; then
    health_host="127.0.0.1"
  elif [ "$health_host" = "::" ]; then
    health_host="[::1]"
  elif [[ "$health_host" == *:* && "$health_host" != \[*\] ]]; then
    health_host="[$health_host]"
  fi
  if [ ! -x "$SCRIPT_DIR/cyanrex-release" ]; then
    echo "Error: the matching packaged cyanrex-release is required for health checks." >&2
    exit 1
  fi
  HEALTH_ARGS=(smoke wait-health --engine-url "http://${health_host}:${health_port}"
    --timeout-seconds "${CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS:-90}"
    --poll-seconds "${CYANREX_DEPLOY_HEALTH_POLL_SECONDS:-2}")
  "$SCRIPT_DIR/cyanrex-release" "${HEALTH_ARGS[@]}" --check-only
}

wait_for_health() {
  if [ "${CYANREX_DEPLOY_WAIT_FOR_HEALTH:-1}" = "0" ]; then
    echo "[cyanrex] Health checking skipped; readiness unverified."
    return
  fi
  if ! "$SCRIPT_DIR/cyanrex-release" "${HEALTH_ARGS[@]}"; then
    echo "Error: services may already be running; no automatic rollback was attempted." >&2
    echo "Inspect ./deploy.sh status and ./deploy.sh logs engine (Ctrl-C to stop following)." >&2
    return 1
  fi
}

print_endpoints() {
  echo "[cyanrex] Service endpoints:"
  echo "  frontend: http://${CYANREX_BIND_ADDRESS:-127.0.0.1}:${CYANREX_FRONTEND_PORT:-3000}"
  echo "  engine:   http://${CYANREX_BIND_ADDRESS:-127.0.0.1}:${CYANREX_ENGINE_PORT:-8080}/health"
  echo "  postgres: ${CYANREX_BIND_ADDRESS:-127.0.0.1}:${CYANREX_POSTGRES_PORT:-15432}"
  echo "  login:    admin (check .env credentials)"
}

check_runtime() {
  require_cmd docker
  if ! docker info >/dev/null 2>&1; then
    echo "Error: docker daemon is not running or not reachable." >&2
    exit 1
  fi
  if ! compose_has_project; then
    echo "Error: invalid compose file: $COMPOSE_FILE" >&2
    exit 1
  fi
}

deploy_up() {
  if [ ! -f "$ENV_FILE" ]; then
    echo "Error: .env is required for deploy." >&2
    echo "Run: cp .env.example .env && edit secrets before deploy." >&2
    exit 1
  fi

  load_env_file

  CYANREX_IMAGE_TAG="${CYANREX_IMAGE_TAG:-$IMAGE_TAG}"
  CYANREX_ENGINE_IMAGE="${CYANREX_ENGINE_IMAGE:-$ENGINE_IMAGE_DEFAULT}"
  CYANREX_FRONTEND_IMAGE="${CYANREX_FRONTEND_IMAGE:-$FRONTEND_IMAGE_DEFAULT}"
  POSTGRES_IMAGE="${POSTGRES_IMAGE:-$POSTGRES_IMAGE_DEFAULT}"

  export CYANREX_IMAGE_TAG CYANREX_ENGINE_IMAGE CYANREX_FRONTEND_IMAGE POSTGRES_IMAGE

  validate_required_secrets
  prepare_health_check
  check_runtime
  load_images_if_missing

  compose up -d "$@"

  wait_for_health
  print_endpoints
}

deploy_down() {
  compose --profile runner-agent down "$@"
}

deploy_status() {
  compose ps
}

deploy_logs() {
  compose logs -f "$@"
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
  CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS=90 Total health wait seconds (1..3600)
  CYANREX_DEPLOY_HEALTH_POLL_SECONDS=2    Health poll interval (1..60 seconds)
USAGE
}

ACTION="${1:-up}"
shift || true

case "$ACTION" in
  up|start)
    deploy_up "$@"
    ;;
  down|stop)
    load_env_file
    check_runtime
    deploy_down "$@"
    ;;
  status)
    load_env_file
    check_runtime
    deploy_status
    ;;
  logs)
    load_env_file
    check_runtime
    deploy_logs "$@"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Error: unknown command '$ACTION'." >&2
    usage
    exit 1
    ;;
esac
