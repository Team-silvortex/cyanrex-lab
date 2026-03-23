#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"
LOCAL_DATABASE_URL="postgres://postgres:postgres@localhost:15432/cyanrex"

usage() {
  cat <<'USAGE'
Usage:
  ./start.sh start [--local] [--rebuild] [--pull] [--no-fallback]
                              Start stack (default: docker fast-start)
  ./start.sh stop              Stop docker stack
  ./start.sh status            Show docker stack status
  ./start.sh logs [service]    Follow docker logs (optional service)

Compatible shortcuts:
  ./start.sh                   Same as: ./start.sh start
  ./start.sh --local           Same as: ./start.sh start --local

Start options:
  --local        Run engine/frontend locally, only postgres in Docker
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
  echo "  postgres: localhost:15432"
  echo "  login:    admin / cyanrex-admin + TOTP secret JBSWY3DPEHPK3PXP"
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
  while IFS= read -r mirror; do
    [ -z "$mirror" ] && continue
    host="${mirror#http://}"
    host="${host#https://}"
    host="${host%%/*}"
    if getent hosts "$host" >/dev/null 2>&1; then
      echo "  [OK]   $mirror"
    else
      echo "  [FAIL] $mirror (DNS unresolved)"
      echo "         Fix: remove/replace this mirror in /etc/docker/daemon.json, then restart docker."
      echo "         Example daemon.json:"
      echo '         { "registry-mirrors": ["https://mirror.gcr.io"] }'
      echo "         Or remove registry-mirrors entirely to use docker.io directly."
      return 1
    fi
  done <<< "$mirrors"
}

start_docker_stack() {
  local force_rebuild="${1:-0}"
  local do_pull="${2:-0}"
  local allow_fallback="${3:-1}"

  require_cmd docker
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
  FRONTEND_NODE_IMAGE="m.daocloud.io/docker.io/library/node:20" \
  POSTGRES_IMAGE="m.daocloud.io/docker.io/library/postgres:16" \
  compose "${up_args[@]}"
  print_endpoints
}

start_local_stack() {
  require_cmd docker
  require_cmd cargo
  require_cmd npm
  run_host_preflight
  check_registry_mirrors

  echo "[cyanrex] Starting postgres with Docker..."
  compose up -d postgres

  echo "[cyanrex] Starting engine locally..."
  (
    cd "$ROOT_DIR/engine"
    ENGINE_HOST=0.0.0.0 \
    ENGINE_PORT=8080 \
    DATABASE_URL="$LOCAL_DATABASE_URL" \
    CYANREX_ADMIN_USERNAME=admin \
    CYANREX_ADMIN_PASSWORD=cyanrex-admin \
    CYANREX_ADMIN_TOTP_SECRET=JBSWY3DPEHPK3PXP \
    cargo run
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
    use_local=0
    force_rebuild=0
    do_pull=0
    allow_fallback=1

    if [ $# -gt 0 ]; then
      shift
    fi
    while [ $# -gt 0 ]; do
      case "$1" in
        --local)
          use_local=1
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

    if [ "$use_local" -eq 1 ]; then
      start_local_stack
    else
      start_docker_stack "$force_rebuild" "$do_pull" "$allow_fallback"
    fi
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
