#!/usr/bin/env bash

acquire_start_lock() {
  local lock_root="${ROOT_DIR}/.run/start-locks"
  START_LOCK_DIR=""
  START_LOCK_ACQUIRED=0
  START_LOCK_MODE=""
  START_LOCK_FILE="$lock_root/${CYANREX_COMPOSE_PROJECT}.lock"

  mkdir -p "$lock_root"
  if command -v flock >/dev/null 2>&1; then
    START_LOCK_MODE="flock"
    : > "$START_LOCK_FILE"
    START_LOCK_FD=9
    eval "exec ${START_LOCK_FD}>\"$START_LOCK_FILE\""
    if ! flock -n "$START_LOCK_FD"; then
      local holder_pid=""
      holder_pid="$(sed -n '1p' "$START_LOCK_FILE" 2>/dev/null | tr -d ' \t\r\n')"
      echo "[ERROR] Another start for instance '${CYANREX_INSTANCE_ID}' is already in progress."
      if [ -n "$holder_pid" ]; then
        echo "  Holder process: ${holder_pid}"
      fi
      exit 1
    fi
    printf '%s\n' "$$" > "$START_LOCK_FILE"
    START_LOCK_ACQUIRED=1
    return
  fi

  START_LOCK_MODE="mkdir"
  START_LOCK_DIR="$lock_root/${CYANREX_COMPOSE_PROJECT}"

  if [ -d "$START_LOCK_DIR" ]; then
    local stale_pid=""
    local existing_pid=""
    existing_pid="$(cat "$START_LOCK_DIR/pid" 2>/dev/null | tr -d ' \t\r\n' || true)"
    if [ -n "$existing_pid" ] && kill -0 "$existing_pid" 2>/dev/null; then
      echo "[ERROR] Another start for instance '${CYANREX_INSTANCE_ID}' is already in progress (pid: ${existing_pid})." >&2
      exit 1
    fi

    stale_pid="$existing_pid"
    if [ -n "$stale_pid" ]; then
      echo "[cyanrex] Removing stale lock from pid ${stale_pid} for instance ${CYANREX_INSTANCE_ID}."
    fi
    rm -rf "$START_LOCK_DIR"
  fi

  if ! mkdir "$START_LOCK_DIR" 2>/dev/null; then
    echo "[ERROR] Failed to acquire startup lock for instance '${CYANREX_INSTANCE_ID}'." >&2
    exit 1
  fi

  printf '%s\n' "$$" > "$START_LOCK_DIR/pid"
  START_LOCK_ACQUIRED=1
}

release_start_lock() {
  if [ "${START_LOCK_ACQUIRED:-0}" -ne 1 ]; then
    return
  fi

  case "${START_LOCK_MODE:-}" in
    flock)
      flock -u "${START_LOCK_FD}" 2>/dev/null || true
      eval "exec ${START_LOCK_FD}>&-" || true
      ;;
    mkdir)
      if [ -n "${START_LOCK_DIR:-}" ] && [ -d "$START_LOCK_DIR" ]; then
        rm -rf "$START_LOCK_DIR"
      fi
      ;;
  esac

  START_LOCK_ACQUIRED=0
}

cleanup_on_exit() {
  if [ "${RUNNING_LOCAL_STACK:-0}" -eq 1 ]; then
    local -a pids=()
    [ -n "${ENGINE_PID:-}" ] && pids+=("$ENGINE_PID")
    [ -n "${FRONTEND_PID:-}" ] && pids+=("$FRONTEND_PID")
    if [ "${#pids[@]}" -gt 0 ]; then
      echo "[cyanrex] Stopping local services..."
      kill "${pids[@]}" 2>/dev/null || true
    fi
  fi
  release_start_lock
}
