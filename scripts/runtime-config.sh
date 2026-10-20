#!/usr/bin/env bash
# Shared source-checkout selectors. Other .env settings retain trusted shell semantics at start.
RUNTIME_SELECTOR_KEYS=(CYANREX_INSTANCE_ID CYANREX_ENGINE_PORT CYANREX_FRONTEND_PORT CYANREX_POSTGRES_PORT CYANREX_BIND_ADDRESS)

runtime_env_read() {
  local mode="$1" key="${2:-}"
  [ -f "$ENV_FILE" ] || return 0
  awk -v mode="$mode" -v wanted="$key" '
    function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
    function selector(k) { return k ~ /^CYANREX_(INSTANCE_ID|ENGINE_PORT|FRONTEND_PORT|POSTGRES_PORT|BIND_ADDRESS|COMPOSE_PROJECT)$/ }
    {
      if (index($0, sprintf("%c", 0))) exit 2
      sub(/\r$/, ""); line = $0; candidate = trim(line); sub(/^export[ \t]+/, "", candidate)
      equals = index(candidate, "="); key = equals ? trim(substr(candidate, 1, equals - 1)) : ""
      if (mode == "shell") { if (!selector(key)) print line; next }
      if (key != wanted) next
      value = trim(substr(candidate, equals + 1)); quote = substr(value, 1, 1)
      if (quote == "\047" || quote == "\042") {
        tail = substr(value, 2); end = index(tail, quote)
        rest = end ? trim(substr(tail, end + 1)) : "invalid"
        if (end && (rest == "" || substr(rest, 1, 1) == "#")) value = substr(tail, 1, end - 1)
      } else { sub(/[ \t]+#.*/, "", value); value = trim(value) }
      found = 1; last = value
    }
    END { if (mode != "shell" && found) printf "%s", last }
  ' "$ENV_FILE" || { echo "Error: cannot read runtime configuration as single-line text." >&2; return 1; }
}

load_runtime_selectors() {
  local key value fallback
  for key in "${RUNTIME_SELECTOR_KEYS[@]}"; do
    case "$key" in
      CYANREX_INSTANCE_ID) fallback=default ;;
      CYANREX_ENGINE_PORT) fallback=8080 ;;
      CYANREX_FRONTEND_PORT) fallback=3000 ;;
      CYANREX_POSTGRES_PORT) fallback=15432 ;;
      CYANREX_BIND_ADDRESS) fallback=127.0.0.1 ;;
    esac
    value="${!key:-}"
    if [ -z "$value" ]; then value="$(runtime_env_read value "$key")"; fi
    printf -v "$key" '%s' "${value:-$fallback}"
  done
}

valid_runtime_port() {
  [[ "$1" =~ ^0*([1-9][0-9]{0,4})$ ]] && [ "${BASH_REMATCH[1]}" -le 65535 ]
}

apply_runtime_identity() {
  if ! [[ "$CYANREX_INSTANCE_ID" =~ ^[a-z0-9_-]+$ ]] || [ "${#CYANREX_INSTANCE_ID}" -gt 63 ]; then
    echo "Error: instance-id must contain 1..63 lowercase letters, digits, underscores or hyphens; no target selected." >&2
    return 1
  fi
  if [ -z "$CYANREX_BIND_ADDRESS" ] || [[ "$CYANREX_BIND_ADDRESS" == *[!a-zA-Z0-9_.:\[\]-]* ]]; then
    echo "Error: bind-address must be a literal address; no target started." >&2
    return 1
  fi
  CYANREX_COMPOSE_PROJECT="cyanrex-${CYANREX_INSTANCE_ID}"
  export CYANREX_COMPOSE_PROJECT
  export "${RUNTIME_SELECTOR_KEYS[@]}"
}

normalize_and_apply_runtime_env() {
  local key
  apply_runtime_identity
  for key in CYANREX_ENGINE_PORT CYANREX_FRONTEND_PORT CYANREX_POSTGRES_PORT; do
    if ! valid_runtime_port "${!key}"; then
      echo "Error: $key must be a literal port in range 1..65535; no target started." >&2
      return 1
    fi
  done
}

runtime_target_snapshot() {
  printf '%s\n' "$CYANREX_INSTANCE_ID" "$CYANREX_ENGINE_PORT" "$CYANREX_FRONTEND_PORT" \
    "$CYANREX_POSTGRES_PORT" "$CYANREX_BIND_ADDRESS" "$CYANREX_COMPOSE_PROJECT"
}

runtime_compose_env_file() {
  if [ -f "$ENV_FILE" ]; then printf '%s' "$ENV_FILE"; else printf '/dev/null'; fi
}
