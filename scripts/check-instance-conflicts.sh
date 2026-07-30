#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_PREFIX="cyanrex-"

usage() {
  cat <<'USAGE'
Usage:
  ./scripts/check-instance-conflicts.sh [--instance-id <id>] [--engine-port <port>]
                                       [--frontend-port <port>] [--postgres-port <port>]
                                       [--bind-address <addr>]
                                       [--allow-existing-running]
                                       [--skip-port-checks-if-running]

Checks:
  - whether this instance ID already has a running compose project
  - whether requested host ports are currently in use

Options:
  --instance-id          Instance ID (default: default)
  --engine-port          Engine publish port (default: 8080)
  --frontend-port        Frontend publish port (default: 3000)
  --postgres-port        Postgres publish port (default: 15432)
  --bind-address         Published bind IP (default: 127.0.0.1)
  --allow-existing-running
                         Skip hard conflict failure when compose project is already running
                         (useful when re-attaching an active instance)
  --skip-port-checks-if-running
                         If project is already running, skip all socket checks
  -h, --help            Show help
USAGE
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

check_listener_with_lsof() {
  local label="$1"
  local port="$2"
  local listeners
  listeners="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
  if [ -z "$listeners" ]; then
    return 0
  fi

  echo "[WARN] ${label} port ${port} is already listened by process(es):"
  echo "$listeners" | sed -n '1,5p'
  return 1
}

check_listener_with_ss() {
  local label="$1"
  local port="$2"
  local listeners
  listeners="$(ss -ltnH "sport = :$port" 2>/dev/null || true)"
  if [ -z "$listeners" ]; then
    return 0
  fi

  echo "[WARN] ${label} port ${port} is already listened by socket(s):"
  echo "$listeners" | sed -n '1,5p'
  return 1
}

check_listener_with_netstat() {
  local label="$1"
  local port="$2"
  local listeners
  listeners="$(netstat -ltn 2>/dev/null | grep -E ":$port[[:space:]]|:$port$" || true)"
  if [ -z "$listeners" ]; then
    return 0
  fi

  echo "[WARN] ${label} port ${port} is already listened by socket(s):"
  echo "$listeners" | sed -n '1,5p'
  return 1
}

check_port_free() {
  local label="$1"
  local port="$2"
  local bind_address="${3:-127.0.0.1}"
  local checked=0

  echo "[cyanrex] Checking ${label} port ${bind_address}:${port} ... "
  if command -v lsof >/dev/null 2>&1; then
    checked=1
    if ! check_listener_with_lsof "$label" "$port"; then
      return 1
    fi
  fi

  if [ "$checked" -eq 0 ] && command -v ss >/dev/null 2>&1; then
    checked=1
    if ! check_listener_with_ss "$label" "$port"; then
      return 1
    fi
  fi

  if [ "$checked" -eq 0 ] && command -v netstat >/dev/null 2>&1; then
    checked=1
    if ! check_listener_with_netstat "$label" "$port"; then
      return 1
    fi
  fi

  if [ "$checked" -eq 0 ]; then
    echo "[WARN] Cannot detect open sockets (no lsof/ss/netstat); skipping ${label} port ${port} check."
  fi

  return 0
}

CYANREX_INSTANCE_ID="${CYANREX_INSTANCE_ID:-default}"
CYANREX_ENGINE_PORT="${CYANREX_ENGINE_PORT:-8080}"
CYANREX_FRONTEND_PORT="${CYANREX_FRONTEND_PORT:-3000}"
CYANREX_POSTGRES_PORT="${CYANREX_POSTGRES_PORT:-15432}"
CYANREX_BIND_ADDRESS="${CYANREX_BIND_ADDRESS:-127.0.0.1}"
ALLOW_RUNNING_PROJECT=0
SKIP_PORT_CHECK_IF_RUNNING=0
check_failed=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --instance-id)
      if [ "$#" -lt 2 ]; then
        echo "Error: --instance-id needs a value." >&2
        exit 1
      fi
      CYANREX_INSTANCE_ID="$2"
      shift 2
      ;;
    --engine-port)
      if [ "$#" -lt 2 ]; then
        echo "Error: --engine-port needs a value." >&2
        exit 1
      fi
      CYANREX_ENGINE_PORT="$2"
      shift 2
      ;;
    --frontend-port)
      if [ "$#" -lt 2 ]; then
        echo "Error: --frontend-port needs a value." >&2
        exit 1
      fi
      CYANREX_FRONTEND_PORT="$2"
      shift 2
      ;;
    --postgres-port)
      if [ "$#" -lt 2 ]; then
        echo "Error: --postgres-port needs a value." >&2
        exit 1
      fi
      CYANREX_POSTGRES_PORT="$2"
      shift 2
      ;;
    --bind-address)
      if [ "$#" -lt 2 ]; then
        echo "Error: --bind-address needs a value." >&2
        exit 1
      fi
      CYANREX_BIND_ADDRESS="$2"
      shift 2
      ;;
    --allow-existing-running)
      ALLOW_RUNNING_PROJECT=1
      shift
      ;;
    --skip-port-checks-if-running)
      SKIP_PORT_CHECK_IF_RUNNING=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

CYANREX_INSTANCE_ID="$(sanitize_instance_id "$CYANREX_INSTANCE_ID")"
validate_port engine "$CYANREX_ENGINE_PORT"
validate_port frontend "$CYANREX_FRONTEND_PORT"
validate_port postgres "$CYANREX_POSTGRES_PORT"
CYANREX_COMPOSE_PROJECT="${PROJECT_PREFIX}${CYANREX_INSTANCE_ID}"

if [ "${CYANREX_ENGINE_PORT}" = "${CYANREX_FRONTEND_PORT}" ] || \
   [ "${CYANREX_ENGINE_PORT}" = "${CYANREX_POSTGRES_PORT}" ] || \
   [ "${CYANREX_FRONTEND_PORT}" = "${CYANREX_POSTGRES_PORT}" ]; then
  echo "[ERROR] Port duplication detected among runtime ports."
  check_failed=1
fi
running_containers=""
if command -v docker >/dev/null 2>&1; then
  running_containers="$(docker ps --filter "label=com.docker.compose.project=${CYANREX_COMPOSE_PROJECT}" --format '{{.Names}}' 2>/dev/null || true)"
  if [ -n "$running_containers" ]; then
    if [ "$ALLOW_RUNNING_PROJECT" -eq 1 ]; then
      echo "[cyanrex] Existing running project detected: ${CYANREX_COMPOSE_PROJECT} (${running_containers//$'\n'/, })"
      if [ "$SKIP_PORT_CHECK_IF_RUNNING" -eq 1 ]; then
        echo "[cyanrex] SKIP_PORT_CHECK_IF_RUNNING enabled; skip listener checks for existing instance."
      fi
    else
      echo "[ERROR] Project ${CYANREX_COMPOSE_PROJECT} is already running:"
      echo "$running_containers"
      echo "If you want to reconnect to an active instance, pass --allow-existing-running."
      check_failed=1
    fi
  fi
fi

if [ "$check_failed" -ne 0 ]; then
  exit 1
fi

if [ -n "$running_containers" ] && [ "$SKIP_PORT_CHECK_IF_RUNNING" -eq 1 ]; then
  echo "[cyanrex] Check complete: no hard conflicts."
  exit 0
fi

check_port_free "engine" "$CYANREX_ENGINE_PORT" "$CYANREX_BIND_ADDRESS" || check_failed=1
check_port_free "frontend" "$CYANREX_FRONTEND_PORT" "$CYANREX_BIND_ADDRESS" || check_failed=1
check_port_free "postgres" "$CYANREX_POSTGRES_PORT" "$CYANREX_BIND_ADDRESS" || check_failed=1

if [ "$check_failed" -ne 0 ]; then
  echo "[ERROR] Conflict check failed for instance ${CYANREX_INSTANCE_ID} (${CYANREX_COMPOSE_PROJECT})."
  exit 1
fi

echo "[cyanrex] Conflict check passed for ${CYANREX_COMPOSE_PROJECT}."
echo "  engine:   ${CYANREX_BIND_ADDRESS}:${CYANREX_ENGINE_PORT}"
echo "  frontend: ${CYANREX_BIND_ADDRESS}:${CYANREX_FRONTEND_PORT}"
echo "  postgres: ${CYANREX_BIND_ADDRESS}:${CYANREX_POSTGRES_PORT}"
exit 0
