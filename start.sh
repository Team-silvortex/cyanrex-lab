#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"

usage() {
  cat <<'USAGE'
Usage:
  ./start.sh           # Start full stack with Docker Compose
  ./start.sh --local   # Start postgres via Docker, engine/frontend locally
USAGE
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required but not installed." >&2
    exit 1
  fi
}

start_docker_stack() {
  require_cmd docker
  echo "[cyanrex] Starting Docker stack..."
  docker compose -f "$COMPOSE_FILE" up --build -d
  echo "[cyanrex] Stack started."
  echo "  frontend: http://localhost:3000"
  echo "  engine:   http://localhost:8080/health"
  echo "  postgres: localhost:5432"
}

start_local_stack() {
  require_cmd docker
  require_cmd cargo
  require_cmd npm

  echo "[cyanrex] Starting postgres with Docker..."
  docker compose -f "$COMPOSE_FILE" up -d postgres

  echo "[cyanrex] Starting engine locally..."
  (
    cd "$ROOT_DIR/engine"
    ENGINE_HOST=0.0.0.0 ENGINE_PORT=8080 DATABASE_URL=postgres://postgres:postgres@localhost:5432/cyanrex cargo run
  ) &
  ENGINE_PID=$!

  echo "[cyanrex] Installing frontend deps if needed..."
  (
    cd "$ROOT_DIR/frontend"
    if [ ! -d node_modules ]; then
      npm install
    fi
    NEXT_PUBLIC_ENGINE_URL=http://localhost:8080 npm run dev
  ) &
  FRONTEND_PID=$!

  trap 'echo "[cyanrex] Stopping local services..."; kill "$ENGINE_PID" "$FRONTEND_PID" 2>/dev/null || true' INT TERM EXIT

  echo "[cyanrex] Local stack started."
  echo "  frontend: http://localhost:3000"
  echo "  engine:   http://localhost:8080/health"
  wait
}

case "${1:-}" in
  "")
    start_docker_stack
    ;;
  --local)
    start_local_stack
    ;;
  -h|--help)
    usage
    ;;
  *)
    echo "Unknown option: $1" >&2
    usage
    exit 1
    ;;
esac
