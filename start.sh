#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"
ENV_FILE="$ROOT_DIR/docker/.env"
source "$ROOT_DIR/scripts/start-lock.sh"

ensure_runtime_secrets() {
  if [ ! -f "$ENV_FILE" ]; then
    require_cmd openssl
    local postgres_password admin_password secret_material totp_secret
    postgres_password="$(openssl rand -hex 24)"
    admin_password="$(openssl rand -base64 24 | tr -d '\n')"
    secret_material="$(openssl rand -base64 64 | tr -dc 'A-Z2-7')"
    totp_secret="${secret_material:0:32}"
    if [ "${#totp_secret}" -lt 32 ]; then
      echo "Error: failed to generate a TOTP secret." >&2
      exit 1
    fi
    umask 077
    printf '%s\n' \
      "POSTGRES_PASSWORD=$postgres_password" \
      "CYANREX_ADMIN_PASSWORD=$admin_password" \
      "CYANREX_ADMIN_TOTP_SECRET=$totp_secret" \
      "CYANREX_BIND_ADDRESS=127.0.0.1" \
      "CYANREX_ALLOW_REGISTRATION=false" \
      "CYANREX_ALLOW_TOTP_BOOTSTRAP=false" \
      "CYANREX_SECURE_COOKIES=false" \
      "CYANREX_ROTATE_ADMIN_CREDENTIALS=false" > "$ENV_FILE"
    echo "[cyanrex] Generated private runtime credentials in docker/.env (mode 0600)."
  fi

  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
  : "${CYANREX_INSTANCE_ID:=default}"
  : "${CYANREX_ENGINE_PORT:=8080}"
  : "${CYANREX_FRONTEND_PORT:=3000}"
  : "${CYANREX_POSTGRES_PORT:=15432}"
  : "${CYANREX_BIND_ADDRESS:=127.0.0.1}"
  : "${CYANREX_COMPOSE_PROJECT:=cyanrex-${CYANREX_INSTANCE_ID}}"
  LOCAL_DATABASE_URL="postgres://postgres:${POSTGRES_PASSWORD}@localhost:${CYANREX_POSTGRES_PORT}/cyanrex"
}

sanitize_instance_id() {
  local value="${1:-default}"
  value="${value//[^a-zA-Z0-9_-]/}"
  if [ -z "$value" ]; then
    value="default"
  fi
  printf "%s" "$value"
}

validate_port() {
  local name="$1"
  local value="$2"
  if ! [[ "$value" =~ ^[0-9]+$ ]]; then
    echo "Error: --${name} must be numeric. Got: ${value}" >&2
    exit 1
  fi
  if [ "$value" -lt 1 ] || [ "$value" -gt 65535 ]; then
    echo "Error: --${name} must be in range 1..65535. Got: ${value}" >&2
    exit 1
  fi
}

normalize_and_apply_runtime_env() {
  CYANREX_INSTANCE_ID="$(sanitize_instance_id "${CYANREX_INSTANCE_ID}")"
  validate_port engine "$CYANREX_ENGINE_PORT"
  validate_port frontend "$CYANREX_FRONTEND_PORT"
  validate_port postgres "$CYANREX_POSTGRES_PORT"
  CYANREX_COMPOSE_PROJECT="cyanrex-${CYANREX_INSTANCE_ID}"
  export CYANREX_INSTANCE_ID
  export CYANREX_ENGINE_PORT
  export CYANREX_FRONTEND_PORT
  export CYANREX_POSTGRES_PORT
  export CYANREX_BIND_ADDRESS
  export CYANREX_COMPOSE_PROJECT
}

parse_runtime_overrides() {
  RUNTIME_ARGS=()
  PARSED_OPTIONS=""
  local runtime_arg_count=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --instance-id)
        if [ $# -lt 2 ]; then
          echo "Error: --instance-id requires a value." >&2
          exit 1
        fi
        CYANREX_INSTANCE_ID="$2"
        PARSED_OPTIONS+=" --instance-id"
        shift 2
        ;;
      --engine-port)
        if [ $# -lt 2 ]; then
          echo "Error: --engine-port requires a port number." >&2
          exit 1
        fi
        CYANREX_ENGINE_PORT="$2"
        PARSED_OPTIONS+=" --engine-port"
        shift 2
        ;;
      --frontend-port)
        if [ $# -lt 2 ]; then
          echo "Error: --frontend-port requires a port number." >&2
          exit 1
        fi
        CYANREX_FRONTEND_PORT="$2"
        PARSED_OPTIONS+=" --frontend-port"
        shift 2
        ;;
      --postgres-port)
        if [ $# -lt 2 ]; then
          echo "Error: --postgres-port requires a port number." >&2
          exit 1
        fi
        CYANREX_POSTGRES_PORT="$2"
        PARSED_OPTIONS+=" --postgres-port"
        shift 2
        ;;
      --bind-address)
        if [ $# -lt 2 ]; then
          echo "Error: --bind-address requires an IP address." >&2
          exit 1
        fi
        CYANREX_BIND_ADDRESS="$2"
        PARSED_OPTIONS+=" --bind-address"
        shift 2
        ;;
      --skip-conflict-check)
        SKIP_CONFLICT_CHECK=1
        PARSED_OPTIONS+=" --skip-conflict-check"
        shift
        ;;
      --skip-start-lock)
        SKIP_START_LOCK=1
        PARSED_OPTIONS+=" --skip-start-lock"
        shift
        ;;
      *)
        RUNTIME_ARGS[runtime_arg_count]="$1"
        runtime_arg_count=$((runtime_arg_count + 1))
        shift
        ;;
    esac
  done
}

usage() {
  cat <<'USAGE'
Usage:
  ./start.sh start [--mode auto|docker|wsl|native] [--instance-id <id>] [--engine-port <port>] [--frontend-port <port>]
                             [--postgres-port <port>] [--bind-address <addr>] [--rebuild] [--pull] [--no-fallback]
                              Start stack (default: docker fast-start)
  ./start.sh stop [--instance-id <id>] [--skip-start-lock]
                             Stop docker stack
  ./start.sh status [--instance-id <id>] [--skip-start-lock]
                               Show docker stack status
  ./start.sh logs [--instance-id <id>] [--skip-start-lock] [service]
                              Follow docker logs (optional service)

Compatible shortcuts:
  ./start.sh                   Same as: ./start.sh start
  ./start.sh --local           Same as: ./start.sh start --local

Start options:
  --mode auto    Docker by default; selects WSL when executed inside WSL2
  --mode docker  Run the full stack in Docker
  --mode wsl     Run engine/frontend natively inside WSL2; PostgreSQL uses Docker
  --mode native  Run engine/frontend on native Linux; PostgreSQL uses Docker
  --local        Compatibility alias for --mode native
  --instance-id   Override cyanrex runtime instance ID (for multi-instance coexistence)
  --engine-port   Override engine listening/published port
  --frontend-port Override frontend published port
  --postgres-port Override postgres published port
  --bind-address  Host IP used for published ports
  --skip-conflict-check
                 Skip runtime conflict precheck (for debug)
  --skip-start-lock
                 Bypass per-instance startup lock (unsafe, debug only)
  --rebuild      Force docker compose build (slower, for Dockerfile/deps changes)
  --pull         Pull latest base images before start (can be slow on poor network)
  --no-fallback  Disable fallback registry retry path

Notes:
  Operations that touch compose state (start/stop/status/logs) are serialized per
  instance ID via a local start lock at:
  .run/start-locks/${CYANREX_COMPOSE_PROJECT}.lock
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required but not installed." >&2
    exit 1
  fi
}

compose() {
  docker compose -p "${CYANREX_COMPOSE_PROJECT}" -f "$COMPOSE_FILE" "$@"
}

print_endpoints() {
  echo "[cyanrex] Ready:"
  echo "  frontend: http://localhost:${CYANREX_FRONTEND_PORT}"
  echo "  engine:   http://localhost:${CYANREX_ENGINE_PORT}/health"
  echo "  postgres: ${CYANREX_BIND_ADDRESS}:${CYANREX_POSTGRES_PORT}"
  echo "  login:    admin (credentials are stored in docker/.env)"
}

run_host_preflight() {
  echo "[cyanrex] Host preflight:"
  if command -v uname >/dev/null 2>&1; then
    echo "  kernel: $(uname -r)"
  fi
  if command -v clang >/dev/null 2>&1; then
    echo "  clang:  $(clang --version | head -n 1)"
  else
    echo "  clang:  missing"
  fi
  if command -v bpftool >/dev/null 2>&1; then
    echo "  bpftool: $(bpftool version | head -n 1)"
  else
    echo "  bpftool: missing"
  fi
  if [ -e /sys/kernel/btf/vmlinux ]; then
    echo "  btf:    /sys/kernel/btf/vmlinux present"
  else
    echo "  btf:    /sys/kernel/btf/vmlinux missing"
  fi
}

detect_host_mode() {
  local release
  release="$(uname -r 2>/dev/null | tr '[:upper:]' '[:lower:]')"
  if [ -n "${WSL_INTEROP:-}" ] || [[ "$release" == *microsoft* ]]; then
    echo "wsl"
  else
    echo "docker"
  fi
}

require_wsl2() {
  if [ "$(detect_host_mode)" != "wsl" ]; then
    echo "Error: --mode wsl must be run inside a WSL2 distribution." >&2
    exit 1
  fi
  if [ ! -e /proc/sys/fs/binfmt_misc/WSLInterop ] && [ -z "${WSL_INTEROP:-}" ]; then
    echo "Error: WSL interoperability is unavailable; WSL2 is required." >&2
    exit 1
  fi
}

check_registry_mirrors() {
  if ! command -v docker >/dev/null 2>&1; then
    return
  fi

  local mirrors
  mirrors="$(docker info --format '{{range .RegistryConfig.Mirrors}}{{println .}}{{end}}' 2>/dev/null || true)"
  if [ -z "$mirrors" ]; then
    return
  fi

  echo "[cyanrex] Docker registry mirror check:"
  local mirror host
  local available=0
  local unavailable=0
  while IFS= read -r mirror; do
    [ -z "$mirror" ] && continue
    host="${mirror#http://}"
    host="${host#https://}"
    host="${host%%/*}"
    if getent hosts "$host" >/dev/null 2>&1; then
      echo "  [OK]   $mirror"
      available=$((available + 1))
    else
      echo "  [FAIL] $mirror (DNS unresolved)"
      unavailable=$((unavailable + 1))
    fi
  done <<< "$mirrors"

  if [ "$unavailable" -gt 0 ]; then
    echo "  Hint: remove or replace unresolved mirrors in /etc/docker/daemon.json."
  fi
  if [ "$available" -eq 0 ] && [ "$unavailable" -gt 0 ]; then
    echo "Error: no configured Docker registry mirror is reachable." >&2
    return 1
  fi
}

run_instance_conflict_check() {
  local runtime_mode="${1:-docker}"
  local -a check_args=(--allow-existing-running)

  if [ "${SKIP_CONFLICT_CHECK:-0}" -eq 1 ]; then
    echo "[cyanrex] Skipping instance conflict check (--skip-conflict-check)."
    return
  fi

  if [ ! -x "$ROOT_DIR/scripts/check-instance-conflicts.sh" ]; then
    echo "[cyanrex] Conflict checker not found; skipping."
    return
  fi

  if [ "$runtime_mode" != "local" ]; then
    check_args+=(--skip-port-checks-if-running)
  fi

  "$ROOT_DIR/scripts/check-instance-conflicts.sh" \
    --instance-id "$CYANREX_INSTANCE_ID" \
    --engine-port "$CYANREX_ENGINE_PORT" \
    --frontend-port "$CYANREX_FRONTEND_PORT" \
    --postgres-port "$CYANREX_POSTGRES_PORT" \
    --bind-address "$CYANREX_BIND_ADDRESS" \
    "${check_args[@]}"
}

start_docker_stack() {
  local force_rebuild="${1:-0}"
  local do_pull="${2:-0}"
  local allow_fallback="${3:-1}"

  require_cmd docker
  ensure_runtime_secrets
  normalize_and_apply_runtime_env
  run_instance_conflict_check docker
  run_host_preflight
  check_registry_mirrors
  if [ "$do_pull" -eq 1 ]; then
    echo "[cyanrex] Pulling base images..."
    compose pull --ignore-pull-failures || true
  fi

  local mode_msg="fast-start"
  local -a up_args=(up -d)
  if [ "$force_rebuild" -eq 1 ]; then
    mode_msg="rebuild"
    up_args=(up --build -d)
  fi

  echo "[cyanrex] Starting Docker stack (${mode_msg})..."
  if compose "${up_args[@]}"; then
    print_endpoints
    return
  fi

  if [ "$allow_fallback" -ne 1 ]; then
    echo "[cyanrex] Start failed and fallback is disabled (--no-fallback)." >&2
    return 1
  fi

  echo "[cyanrex] Primary registry path failed, retrying with fallback registry..."
  ENGINE_RUST_IMAGE="m.daocloud.io/docker.io/library/rust:bookworm" \
  ENGINE_DEBIAN_IMAGE="m.daocloud.io/docker.io/library/debian:bookworm" \
  ENGINE_APT_MIRROR="mirrors.aliyun.com" \
  ENGINE_CARGO_REGISTRY_INDEX="sparse+https://rsproxy.cn/index/" \
  FRONTEND_NODE_IMAGE="m.daocloud.io/docker.io/library/node:20" \
  FRONTEND_NPM_REGISTRY="https://registry.npmmirror.com" \
  POSTGRES_IMAGE="m.daocloud.io/docker.io/library/postgres:16" \
  compose "${up_args[@]}"
  print_endpoints
}

start_local_stack() {
  local runtime_mode="${1:-native}"
  require_cmd docker
  require_cmd cargo
  require_cmd npm
  require_cmd sudo
  ensure_runtime_secrets
  normalize_and_apply_runtime_env
  run_instance_conflict_check local
  RUNNING_LOCAL_STACK=1
  run_host_preflight
  check_registry_mirrors

  echo "[cyanrex] Starting postgres with Docker..."
  compose up -d postgres

  echo "[cyanrex] Building engine locally..."
  cargo build --manifest-path "$ROOT_DIR/engine/Cargo.toml"

  echo "[cyanrex] Starting engine with Linux eBPF privileges..."
  (
    sudo env \
    ENGINE_HOST=0.0.0.0 \
    ENGINE_PORT="${CYANREX_ENGINE_PORT}" \
    CYANREX_INSTANCE_ID="${CYANREX_INSTANCE_ID}" \
    CYANREX_FRONTEND_PORT="${CYANREX_FRONTEND_PORT}" \
    CYANREX_BIND_ADDRESS="${CYANREX_BIND_ADDRESS}" \
    DATABASE_URL="$LOCAL_DATABASE_URL" \
    CYANREX_ADMIN_USERNAME=admin \
    CYANREX_ADMIN_PASSWORD="$CYANREX_ADMIN_PASSWORD" \
    CYANREX_ADMIN_TOTP_SECRET="$CYANREX_ADMIN_TOTP_SECRET" \
    CYANREX_ALLOW_REGISTRATION="${CYANREX_ALLOW_REGISTRATION:-false}" \
    CYANREX_ALLOW_TOTP_BOOTSTRAP="${CYANREX_ALLOW_TOTP_BOOTSTRAP:-false}" \
    CYANREX_RUNTIME_MODE="$runtime_mode" \
    "$ROOT_DIR/engine/target/debug/cyanrex-engine"
  ) &
  ENGINE_PID=$!

  echo "[cyanrex] Starting frontend locally..."
  (
    cd "$ROOT_DIR/frontend"
    if [ ! -d node_modules ]; then
      npm install
    fi
    NEXT_PUBLIC_ENGINE_URL="http://localhost:${CYANREX_ENGINE_PORT}" \
    PORT="${CYANREX_FRONTEND_PORT}" npm run dev
  ) &
  FRONTEND_PID=$!

  print_endpoints
  wait
}

stop_stack() {
  require_cmd docker
  echo "[cyanrex] Stopping Docker stack..."
  compose down
}

status_stack() {
  require_cmd docker
  compose ps
}

logs_stack() {
  require_cmd docker
  if [ $# -gt 0 ]; then
    compose logs -f "$1"
  else
    compose logs -f
  fi
}

assert_supported_options() {
  local command_name="$1"
  shift
  local supported_options=" $* "
  local used_option
  for used_option in ${PARSED_OPTIONS}; do
    case "$supported_options" in
      *" $used_option "*) ;;
      *)
        echo "Error: ${command_name} does not support option: ${used_option}" >&2
        usage
        exit 1
        ;;
    esac
  done
}

with_instance_lock() {
  trap cleanup_on_exit INT TERM EXIT
  if [ "${SKIP_START_LOCK:-0}" -ne 1 ]; then
    acquire_start_lock
  else
    echo "[cyanrex] start-lock bypassed by --skip-start-lock."
  fi
}

extract_log_service_arg() {
  LOG_SERVICE=""
  if [ "${#RUNTIME_ARGS[@]}" -eq 0 ]; then
    return
  fi
  if [ "${#RUNTIME_ARGS[@]}" -gt 1 ]; then
    echo "Unknown arguments for logs: ${RUNTIME_ARGS[*]}" >&2
    usage
    exit 1
  fi
  LOG_SERVICE="${RUNTIME_ARGS[0]}"
}

action="${1:-start}"

if [ "$action" = "--local" ]; then
  action="start"
  set -- "start" "--local"
fi

CYANREX_INSTANCE_ID="${CYANREX_INSTANCE_ID:-default}"
CYANREX_ENGINE_PORT="${CYANREX_ENGINE_PORT:-8080}"
CYANREX_FRONTEND_PORT="${CYANREX_FRONTEND_PORT:-3000}"
CYANREX_POSTGRES_PORT="${CYANREX_POSTGRES_PORT:-15432}"
CYANREX_BIND_ADDRESS="${CYANREX_BIND_ADDRESS:-127.0.0.1}"
SKIP_CONFLICT_CHECK="${SKIP_CONFLICT_CHECK:-0}"
SKIP_START_LOCK="${SKIP_START_LOCK:-0}"
RUNTIME_ARGS=()
if [ "$#" -gt 0 ]; then
  shift
fi
parse_runtime_overrides "$@"
normalize_and_apply_runtime_env

case "$action" in
  start)
    runtime_mode="docker"
    force_rebuild=0
    do_pull=0
    allow_fallback=1
    with_instance_lock

    while [ "${#RUNTIME_ARGS[@]}" -gt 0 ]; do
      case "${RUNTIME_ARGS[0]}" in
        --local)
          runtime_mode="native"
          RUNTIME_ARGS=("${RUNTIME_ARGS[@]:1}")
          ;;
        --mode)
          if [ "${#RUNTIME_ARGS[@]}" -lt 2 ]; then
            echo "Error: --mode requires auto, docker, wsl, or native." >&2
            exit 1
          fi
          runtime_mode="${RUNTIME_ARGS[1]}"
          RUNTIME_ARGS=("${RUNTIME_ARGS[@]:2}")
          ;;
        --rebuild)
          force_rebuild=1
          RUNTIME_ARGS=("${RUNTIME_ARGS[@]:1}")
          ;;
        --pull)
          do_pull=1
          RUNTIME_ARGS=("${RUNTIME_ARGS[@]:1}")
          ;;
        --no-fallback)
          allow_fallback=0
          RUNTIME_ARGS=("${RUNTIME_ARGS[@]:1}")
          ;;
        *)
          echo "Unknown option for start: ${RUNTIME_ARGS[0]}" >&2
          usage
          exit 1
          ;;
      esac
    done

    if [ "$runtime_mode" = "auto" ]; then
      runtime_mode="$(detect_host_mode)"
    fi
    case "$runtime_mode" in
      docker)
        start_docker_stack "$force_rebuild" "$do_pull" "$allow_fallback"
        ;;
      wsl)
        require_wsl2
        start_local_stack wsl2
        ;;
      native)
        start_local_stack native-linux
        ;;
      *)
        echo "Error: unsupported runtime mode '$runtime_mode'." >&2
        exit 1
        ;;
    esac
  ;;
  stop)
    assert_supported_options "stop" --instance-id --skip-start-lock
    with_instance_lock
    if [ "${#RUNTIME_ARGS[@]}" -gt 0 ]; then
      echo "Unknown argument(s) for stop: ${RUNTIME_ARGS[*]}" >&2
      usage
      exit 1
    fi
    stop_stack
    ;;
  status)
    assert_supported_options "status" --instance-id --skip-start-lock
    with_instance_lock
    if [ "${#RUNTIME_ARGS[@]}" -gt 0 ]; then
      echo "Unknown argument(s) for status: ${RUNTIME_ARGS[*]}" >&2
      usage
      exit 1
    fi
    status_stack
    ;;
  logs)
    assert_supported_options "logs" --instance-id --skip-start-lock
    with_instance_lock
    extract_log_service_arg
    logs_stack "$LOG_SERVICE"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Unknown command: $action" >&2
    usage
    exit 1
    ;;
esac
