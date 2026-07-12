#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"
ENV_FILE="$ROOT_DIR/docker/.env"

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
  LOCAL_DATABASE_URL="postgres://postgres:${POSTGRES_PASSWORD}@localhost:15432/cyanrex"
}

usage() {
  cat <<'USAGE'
Usage:
  ./start.sh start [--mode auto|docker|wsl|native] [--rebuild] [--pull] [--no-fallback]
                              Start stack (default: docker fast-start)
  ./start.sh stop              Stop docker stack
  ./start.sh status            Show docker stack status
  ./start.sh logs [service]    Follow docker logs (optional service)

Compatible shortcuts:
  ./start.sh                   Same as: ./start.sh start
  ./start.sh --local           Same as: ./start.sh start --local

Start options:
  --mode auto    Docker by default; selects WSL when executed inside WSL2
  --mode docker  Run the full stack in Docker
  --mode wsl     Run engine/frontend natively inside WSL2; PostgreSQL uses Docker
  --mode native  Run engine/frontend on native Linux; PostgreSQL uses Docker
  --local        Compatibility alias for --mode native
  --rebuild      Force docker compose build (slower, for Dockerfile/deps changes)
  --pull         Pull latest base images before start (can be slow on poor network)
  --no-fallback  Disable fallback registry retry path
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required but not installed." >&2
    exit 1
  fi
}

compose() {
  docker compose -f "$COMPOSE_FILE" "$@"
}

print_endpoints() {
  echo "[cyanrex] Ready:"
  echo "  frontend: http://localhost:3000"
  echo "  engine:   http://localhost:8080/health"
  echo "  postgres: 127.0.0.1:15432"
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

start_docker_stack() {
  local force_rebuild="${1:-0}"
  local do_pull="${2:-0}"
  local allow_fallback="${3:-1}"

  require_cmd docker
  ensure_runtime_secrets
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
    ENGINE_PORT=8080 \
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
    NEXT_PUBLIC_ENGINE_URL=http://localhost:8080 npm run dev
  ) &
  FRONTEND_PID=$!

  trap 'echo "[cyanrex] Stopping local services..."; kill "$ENGINE_PID" "$FRONTEND_PID" 2>/dev/null || true' INT TERM EXIT

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

action="${1:-start}"

if [ "$action" = "--local" ]; then
  action="start"
  set -- "start" "--local"
fi

case "$action" in
  start)
    runtime_mode="docker"
    force_rebuild=0
    do_pull=0
    allow_fallback=1

    if [ $# -gt 0 ]; then
      shift
    fi
    while [ $# -gt 0 ]; do
      case "$1" in
        --local)
          runtime_mode="native"
          ;;
        --mode)
          if [ $# -lt 2 ]; then
            echo "Error: --mode requires auto, docker, wsl, or native." >&2
            exit 1
          fi
          runtime_mode="$2"
          shift
          ;;
        --rebuild)
          force_rebuild=1
          ;;
        --pull)
          do_pull=1
          ;;
        --no-fallback)
          allow_fallback=0
          ;;
        *)
          echo "Unknown option for start: $1" >&2
          usage
          exit 1
          ;;
      esac
      shift
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
    stop_stack
    ;;
  status)
    status_stack
    ;;
  logs)
    logs_stack "${2:-}"
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
