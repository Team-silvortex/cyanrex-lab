#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="all"
SKIP_NPM_INSTALL=0
RUN_SECURITY_AUDIT=0

print_help() {
  cat <<'EOF'
Usage: ./scripts/quality-gate.sh [--backend-only|--frontend-only|--format-only|--security|--security-only|--no-npm-install]

Runs project quality checks used before commit/CI.

Modes:
  (default)      Run file-length, backend, and frontend checks.
  --backend-only Run file-length and backend checks.
  --frontend-only Run file-length and frontend checks.
  --format-only  Run file-length and Rust formatting check only.
  --security     Run file-length and security audit check.
  --security-only Run only security audit check.

Flags:
  --no-npm-install Skip npm install step in frontend checks.
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
    --format-only)
      MODE="format-only"
      ;;
    --security)
      RUN_SECURITY_AUDIT=1
      ;;
    --security-only)
      MODE="security-only"
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

run_backend_checks() {
  cargo fmt --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" -- --check
  cargo test --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" --locked
}

run_security_check() {
  "$PROJECT_ROOT/scripts/check-security-audit.sh"
}

run_frontend_checks() {
  if [[ "$SKIP_NPM_INSTALL" == "1" ]]; then
    echo "Skipping npm install for frontend checks."
  else
    npm ci --prefix "$PROJECT_ROOT/frontend"
  fi

  npm --prefix "$PROJECT_ROOT/frontend" run build
  (cd "$PROJECT_ROOT/frontend" && npx --yes tsc --noEmit)
}

run_format_only() {
  cargo fmt --manifest-path "$PROJECT_ROOT/engine/Cargo.toml" -- --check
}

run_file_length_check

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
  format-only)
    run_format_only
    ;;
  all)
    if [[ "$RUN_SECURITY_AUDIT" == "1" ]]; then
      run_security_check
    fi
    run_backend_checks
    run_frontend_checks
    ;;
esac
