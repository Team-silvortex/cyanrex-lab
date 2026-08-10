#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/../docker/docker-compose.yml" ]; then
  ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
elif [ -f "$SCRIPT_DIR/docker-compose.yml" ]; then
  ROOT_DIR="$SCRIPT_DIR"
else
  echo "Error: cannot locate the Cyanrex Compose configuration." >&2
  exit 1
fi
if [ -f "$ROOT_DIR/docker/docker-compose.yml" ]; then
  COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"
  ENV_FILE="$ROOT_DIR/docker/.env"
  TOKEN_FILE="$ROOT_DIR/docker/.runner-agent-token"
  SOURCE_TREE=1
else
  COMPOSE_FILE="$ROOT_DIR/docker-compose.yml"
  ENV_FILE="$ROOT_DIR/.env"
  TOKEN_FILE="$ROOT_DIR/.runner-agent-token"
  SOURCE_TREE=0
fi
COMPOSE_CMD=()
COMPOSE_PROJECT_ARGS=()
TOKEN_CHANGED=0

usage() {
  cat <<'EOF'
Usage: ./scripts/runner-agent.sh start|stop|status|logs|prepare [--agent-id <id>]

Starts the optional unprivileged Docker compile Agent. The main stack configuration must exist.
`prepare` creates the private Docker Secret without starting containers.
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: '$1' is required." >&2
    exit 1
  fi
}

resolve_compose() {
  if [ "${#COMPOSE_CMD[@]}" -gt 0 ]; then return; fi
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
  else
    echo "Error: Docker Compose is unavailable." >&2
    exit 1
  fi
}

compose() {
  resolve_compose
  "${COMPOSE_CMD[@]}" "${COMPOSE_PROJECT_ARGS[@]}" -f "$COMPOSE_FILE" --profile runner-agent "$@"
}

load_configuration() {
  if [ ! -f "$ENV_FILE" ]; then
    echo "Error: runtime configuration is missing: $ENV_FILE" >&2
    if [ "$SOURCE_TREE" -eq 1 ]; then
      echo "Run ./start.sh start --mode docker first." >&2
    else
      echo "Copy .env.example to .env, replace its placeholders, then run ./run.sh first." >&2
    fi
    exit 1
  fi
  if [ -L "$ENV_FILE" ]; then
    echo "Error: refusing to update symlinked runtime configuration: $ENV_FILE" >&2
    exit 1
  fi
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
  if [ -f "$ROOT_DIR/manifest.env" ]; then
    # shellcheck disable=SC1091
    source "$ROOT_DIR/manifest.env"
    CYANREX_ENGINE_IMAGE="${CYANREX_ENGINE_IMAGE:-${ENGINE_IMAGE:-}}"
    CYANREX_IMAGE_TAG="${CYANREX_IMAGE_TAG:-${PACKAGE_VERSION:-latest}}"
    export CYANREX_ENGINE_IMAGE CYANREX_IMAGE_TAG POSTGRES_IMAGE
  fi
  if [ "$SOURCE_TREE" -eq 1 ]; then
    COMPOSE_PROJECT_ARGS=(-p "cyanrex-${CYANREX_INSTANCE_ID:-default}")
  fi
}

validate_agent_id() {
  if ! [[ "$CYANREX_AGENT_ID" =~ ^[A-Za-z0-9_.-]{3,64}$ ]]; then
    echo "Error: Agent ID must contain 3-64 safe characters." >&2
    exit 1
  fi
}

replace_env_value() {
  local key="$1"
  local value="$2"
  local temp_file
  temp_file="$(mktemp "${ENV_FILE}.tmp.XXXXXX")"
  awk -v key="$key" -v value="$value" '
    BEGIN { replaced = 0 }
    index($0, key "=") == 1 {
      if (!replaced) print key "=" value
      replaced = 1
      next
    }
    { print }
    END { if (!replaced) print key "=" value }
  ' "$ENV_FILE" > "$temp_file"
  chmod 600 "$temp_file"
  mv -f "$temp_file" "$ENV_FILE"
}

prepare_token() {
  local token="${CYANREX_RUNNER_AGENT_TOKEN:-}"
  local temp_token_file
  CYANREX_AGENT_RUNTIME_UID="$(id -u)"
  CYANREX_AGENT_RUNTIME_GID="$(id -g)"
  replace_env_value CYANREX_AGENT_RUNTIME_UID "$CYANREX_AGENT_RUNTIME_UID"
  replace_env_value CYANREX_AGENT_RUNTIME_GID "$CYANREX_AGENT_RUNTIME_GID"
  if [ -n "$CLI_AGENT_ID" ]; then
    replace_env_value CYANREX_AGENT_ID "$CYANREX_AGENT_ID"
  fi
  if [ "${#token}" -lt 32 ] || [ "${#token}" -gt 512 ]; then
    require_cmd openssl
    token="$(openssl rand -hex 32)"
    replace_env_value CYANREX_RUNNER_AGENT_TOKEN "$token"
    TOKEN_CHANGED=1
    echo "[cyanrex] Generated the Runner Agent bootstrap token in the private runtime env."
  fi
  if [ -L "$TOKEN_FILE" ]; then
    echo "Error: refusing to replace symlinked token file: $TOKEN_FILE" >&2
    exit 1
  fi
  umask 077
  chmod 600 "$ENV_FILE"
  temp_token_file="$(mktemp "${TOKEN_FILE}.tmp.XXXXXX")"
  printf '%s\n' "$token" > "$temp_token_file"
  chmod 600 "$temp_token_file"
  mv -f "$temp_token_file" "$TOKEN_FILE"
  CYANREX_RUNNER_AGENT_TOKEN="$token"
  CYANREX_RUNNER_AGENT_TOKEN_FILE="$TOKEN_FILE"
  export CYANREX_RUNNER_AGENT_TOKEN CYANREX_RUNNER_AGENT_TOKEN_FILE CYANREX_AGENT_ID
  export CYANREX_AGENT_RUNTIME_UID CYANREX_AGENT_RUNTIME_GID
}

action="${1:-start}"
if [ "$#" -gt 0 ]; then shift; fi
CLI_AGENT_ID=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --agent-id)
      [ "$#" -ge 2 ] || { echo "Error: --agent-id requires a value." >&2; exit 1; }
      CLI_AGENT_ID="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown option '$1'." >&2
      usage
      exit 1
      ;;
  esac
done

if [[ "$action" =~ ^(-h|--help|help)$ ]]; then
  usage
  exit 0
fi
load_configuration
CYANREX_AGENT_ID="${CLI_AGENT_ID:-${CYANREX_AGENT_ID:-cyanrex-docker-compiler}}"
validate_agent_id

case "$action" in
  prepare)
    prepare_token
    echo "[cyanrex] Runner Agent secret is ready: $TOKEN_FILE"
    ;;
  start)
    require_cmd docker
    prepare_token
    if [ "$TOKEN_CHANGED" -eq 1 ]; then
      echo "[cyanrex] Applying the newly enabled Agent control plane to Engine..."
      compose up -d --force-recreate engine
    else
      compose up -d engine
    fi
    compose up -d runner-agent
    echo "[cyanrex] Runner Agent started as '$CYANREX_AGENT_ID'."
    if [ "$SOURCE_TREE" -eq 1 ]; then
      echo "[cyanrex] Run ./scripts/runner-agent-smoke.sh to verify remote diagnostics."
    else
      echo "[cyanrex] Run ./runner-agent-smoke.sh to verify remote diagnostics."
    fi
    ;;
  stop)
    require_cmd docker
    compose stop runner-agent
    ;;
  status)
    require_cmd docker
    compose ps runner-agent
    ;;
  logs)
    require_cmd docker
    compose logs -f runner-agent
    ;;
  *)
    echo "Error: unknown action '$action'." >&2
    usage
    exit 1
    ;;
esac
