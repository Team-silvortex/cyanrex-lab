#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="all"
SKIP_NPM_INSTALL=0
RUN_SECURITY_AUDIT=0

print_help() {
  cat <<'EOF'
Usage: ./scripts/quality-gate.sh [--backend-only|--frontend-only|--sdk-only|--format-only|--security|--security-only|--permissions-only|--no-npm-install]

Runs project quality checks used before commit/CI. Every mode starts with file-length, version and
OpenAPI drift, Runner Agent tooling, and distribution tooling checks.

Modes:
  (default)      Run common preflight, backend, frontend, and SDK checks.
  --backend-only Run common preflight and backend checks.
  --frontend-only Run common preflight and frontend checks.
  --sdk-only    Run common preflight and JavaScript SDK checks.
  --format-only  Run common preflight and Rust formatting check.
  --security     Add the security audit to the default full checks.
  --security-only Run common preflight and the security audit.
  --permissions-only Run common preflight and permission regressions (backend route_tdd + frontend permission tests).

Flags:
  --no-npm-install Skip npm install steps in frontend and SDK checks.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --backend-only)
      MODE="backend"
      ;;
    --frontend-only)
      MODE="frontend"
      ;;
    --sdk-only)
      MODE="sdk"
      ;;
    --format-only)
      MODE="format-only"
      ;;
    --security)
      RUN_SECURITY_AUDIT=1
      ;;
    --security-only)
      MODE="security-only"
      ;;
    --permissions-only)
      MODE="permissions"
      ;;
    --no-npm-install)
      SKIP_NPM_INSTALL=1
      ;;
    -h|--help)
      print_help
      exit 0
      ;;
    *)
      echo "Unknown argument: $1"
      print_help
      exit 1
      ;;
  esac
  shift
done

run_file_length_check() {
  "$PROJECT_ROOT/scripts/check-file-lengths.sh"
}

run_version_sync_check() {
  "$PROJECT_ROOT/scripts/check-version-sync.sh"
}

run_openapi_contract_checks() {
  node "$PROJECT_ROOT/scripts/generate-openapi.mjs" --check
  node "$PROJECT_ROOT/scripts/openapi-contract.mjs"
  node --test "$PROJECT_ROOT/scripts/tests/openapiContract.test.mjs"
}

run_runner_agent_tool_checks() {
  "$PROJECT_ROOT/scripts/test-runner-agent-tools.sh"
}

run_distribution_tool_checks() {
  "$PROJECT_ROOT/scripts/test-distribution-tools.sh"
}

run_backend_checks() {
  cargo fmt --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" -- --check
  cargo test --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" --locked
}

run_security_check() {
  "$PROJECT_ROOT/scripts/check-security-audit.sh"
}

run_frontend_checks() {
  run_frontend_dependencies

  npm --prefix "$PROJECT_ROOT/frontend" run build
  (cd "$PROJECT_ROOT/frontend" && npx --yes tsc --noEmit)
  npm --prefix "$PROJECT_ROOT/frontend" run test:performance-hotspots
  npm --prefix "$PROJECT_ROOT/frontend" run test:security-headers
  npm --prefix "$PROJECT_ROOT/frontend" run test:tooling
  npm --prefix "$PROJECT_ROOT/frontend" run test:ui-permissions
  npm --prefix "$PROJECT_ROOT/frontend" audit --omit=dev --audit-level=moderate
}

run_frontend_dependencies() {
  if [[ "$SKIP_NPM_INSTALL" == "1" ]]; then
    echo "Skipping npm install for frontend checks."
  else
    npm ci --prefix "$PROJECT_ROOT/frontend"
  fi
}

run_sdk_checks() {
  if [[ "$SKIP_NPM_INSTALL" == "1" ]]; then
    echo "Skipping npm install for SDK checks."
  else
    npm ci --prefix "$PROJECT_ROOT/sdk-js"
  fi
  npm --prefix "$PROJECT_ROOT/sdk-js" run check
  npm --prefix "$PROJECT_ROOT/sdk-js" audit --omit=dev --audit-level=moderate
}

run_format_only() {
  cargo fmt --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" -- --check
}

run_permissions_backend_check() {
  cargo test --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" --test routes_tdd -- --nocapture
}

run_permissions_frontend_check() {
  run_frontend_dependencies
  npm --prefix "$PROJECT_ROOT/frontend" run test:ui-permissions
}

run_permissions_checks() {
  run_permissions_backend_check
  run_permissions_frontend_check
}

run_file_length_check
run_version_sync_check
run_openapi_contract_checks
run_runner_agent_tool_checks
run_distribution_tool_checks

case "$MODE" in
  backend)
    run_backend_checks
    ;;
  security-only)
    run_security_check
    ;;
  frontend)
    run_frontend_checks
    ;;
  sdk)
    run_sdk_checks
    ;;
  permissions)
    run_permissions_checks
    ;;
  format-only)
    run_format_only
    ;;
  all)
    if [[ "$RUN_SECURITY_AUDIT" == "1" ]]; then
      run_security_check
    fi
    run_backend_checks
    run_frontend_checks
    run_sdk_checks
    ;;
esac
