#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$PROJECT_ROOT/docker/docker-compose.yml"
ENV_FILE="$PROJECT_ROOT/docker/.env"

log() {
  echo "[cyanrex-debug] $*"
}

warn() {
  echo "[cyanrex-debug][warn] $*" >&2
}

detect_compose_cmd() {
  if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
    return
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
    return
  fi

  COMPOSE_CMD=()
}

compose() {
  if [ "${#COMPOSE_CMD[@]}" -eq 0 ]; then
    warn "No compose command available."
    return 1
  fi
  "${COMPOSE_CMD[@]}" "$@"
}

run_check() {
  log "run: $*"
  "$@" || warn "command failed: $*"
}

env_value() {
  local key="$1"
  if [ -f "$ENV_FILE" ]; then
    grep "^${key}=" "$ENV_FILE" | tail -n 1 | cut -d= -f2-
    return
  fi
  printf "undefined"
}

value_or_default() {
  local value="$1"
  local default="$2"
  if [ -z "$value" ] || [ "$value" = "undefined" ]; then
    printf "%s" "$default"
  else
    printf "%s" "$value"
  fi
}

instance="$(value_or_default "${CYANREX_INSTANCE_ID:-$(env_value CYANREX_INSTANCE_ID)}" "default")"
instance="$(printf "%s" "$instance" | tr -cd 'A-Za-z0-9_-')"
project="$(value_or_default "${CYANREX_COMPOSE_PROJECT:-$(env_value CYANREX_COMPOSE_PROJECT)}" "cyanrex-${instance}")"
engine_port="$(value_or_default "${CYANREX_ENGINE_PORT:-$(env_value CYANREX_ENGINE_PORT)}" "8080")"
frontend_port="$(value_or_default "${CYANREX_FRONTEND_PORT:-$(env_value CYANREX_FRONTEND_PORT)}" "3000")"
postgres_port="$(value_or_default "${CYANREX_POSTGRES_PORT:-$(env_value CYANREX_POSTGRES_PORT)}" "15432")"
bind_address="$(value_or_default "${CYANREX_BIND_ADDRESS:-$(env_value CYANREX_BIND_ADDRESS)}" "127.0.0.1")"

log "Starting diagnostics for instance '${instance}' (project '${project}')"
log "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
log "Project root: $PROJECT_ROOT"
log "Compose file: $COMPOSE_FILE"
log "Runtime env file: $(if [ -f "$ENV_FILE" ]; then echo "present"; else echo "missing"; fi)"

log "Toolchain:"
if command -v docker >/dev/null 2>&1; then
  log "  docker: $(docker --version | head -n1)"
else
  warn "docker is not available"
fi
if command -v docker-compose >/dev/null 2>&1; then
  log "  docker-compose: $(docker-compose --version | head -n1)"
fi
if command -v clang >/dev/null 2>&1; then
  log "  clang: $(clang --version | head -n1)"
fi
if command -v cargo >/dev/null 2>&1; then
  log "  cargo: $(cargo --version | head -n1)"
fi
if command -v npm >/dev/null 2>&1; then
  log "  npm: $(npm --version)"
fi
if command -v node >/dev/null 2>&1; then
  log "  node: $(node --version)"
fi
if command -v bpftool >/dev/null 2>&1; then
  log "  bpftool: $(bpftool version | head -n1)"
fi

log "Kernel state:"
if command -v uname >/dev/null 2>&1; then
  log "  $(uname -srm)"
fi
if [ -e /sys/kernel/btf/vmlinux ]; then
  log "  btf: present"
else
  warn "  btf: /sys/kernel/btf/vmlinux missing"
fi
if [ -d /sys/fs/bpf ]; then
  log "  bpffs: mounted ($(ls -ld /sys/fs/bpf))"
else
  warn "  bpffs missing"
fi

detect_compose_cmd
if [ "${#COMPOSE_CMD[@]}" -gt 0 ] && [ -f "$ENV_FILE" ]; then
  log "Compose backend: ${COMPOSE_CMD[*]}"
  run_check "${COMPOSE_CMD[@]}" -p "$project" -f "$COMPOSE_FILE" ps
  run_check "${COMPOSE_CMD[@]}" -p "$project" -f "$COMPOSE_FILE" config --services
elif [ "${#COMPOSE_CMD[@]}" -gt 0 ]; then
  warn "docker/.env missing; skipping compose interpolation-sensitive checks"
else
  warn "compose backend unavailable"
fi

for port_name in engine:$engine_port frontend:$frontend_port postgres:$postgres_port; do
  label="${port_name%%:*}"
  port="${port_name#*:}"
  if ! [[ "$port" =~ ^[0-9]+$ ]]; then
    warn "skip invalid port for ${label}: ${port}"
    continue
  fi
  check_port="${bind_address}:${port}"
  log "Checking ${label} port ${check_port}"
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$port" -sTCP:LISTEN || warn "  no listener for ${check_port}"
  elif command -v ss >/dev/null 2>&1; then
    ss -ltnH "sport = :$port" || warn "  no listener for ${check_port}"
  elif command -v netstat >/dev/null 2>&1; then
    netstat -ltn | grep -E ":$port([[:space:]]|$)" || warn "  no listener for ${check_port}"
  else
    warn "  no socket checker (lsof/ss/netstat) available"
  fi
done

if command -v curl >/dev/null 2>&1; then
  run_check curl -fsS "http://127.0.0.1:${engine_port}/health"
else
  warn "curl not installed"
fi

if [ -f "$ENV_FILE" ]; then
  log "Important env values:"
  for key in CYANREX_INSTANCE_ID CYANREX_ENGINE_PORT CYANREX_FRONTEND_PORT CYANREX_POSTGRES_PORT CYANREX_BIND_ADDRESS CYANREX_COMPOSE_PROJECT; do
    value="$(env_value "$key")"
    log "  ${key}=${value}"
  done
fi

if command -v git >/dev/null 2>&1; then
  log "Git state: $(git -C "$PROJECT_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown) @ $(git -C "$PROJECT_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
fi

log "Diagnostics completed."
