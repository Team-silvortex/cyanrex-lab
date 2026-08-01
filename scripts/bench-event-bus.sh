#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVENTS="${CYANREX_BENCH_EVENTS:-200000}"
USERS="${CYANREX_BENCH_USERS:-8}"
CONCURRENCY="${CYANREX_BENCH_CONCURRENCY:-32}"
PAYLOAD_SIZE="${CYANREX_BENCH_PAYLOAD_SIZE:-128}"
MAX_RECORDS="${CYANREX_BENCH_MAX_RECORDS:-20000}"
POLICY="${CYANREX_BENCH_POLICY:-drop_oldest}"
BROADCAST_BUFFER="${CYANREX_BENCH_BROADCAST_BUFFER:-1024}"
VERIFY="${CYANREX_BENCH_VERIFY:-0}"
VERIFY_TIMEOUT="${CYANREX_BENCH_VERIFY_TIMEOUT:-30}"
DATABASE_URL="${CYANREX_BENCH_DATABASE_URL:-}"
OUTPUT_JSON="${CYANREX_BENCH_JSON:-0}"
OUTPUT_JSON_FILE="${CYANREX_BENCH_JSON_FILE:-}"
LABEL="${CYANREX_BENCH_LABEL:-}"

if [ $# -gt 0 ]; then
  case "${1:-}" in
    -h|--help)
      cat <<'EOF'
Run Cyanrex event bus benchmark:
  CYANREX_BENCH_EVENTS=200000
  CYANREX_BENCH_USERS=8
  CYANREX_BENCH_CONCURRENCY=32
  CYANREX_BENCH_PAYLOAD_SIZE=128
  CYANREX_BENCH_MAX_RECORDS=20000
  CYANREX_BENCH_POLICY=drop_oldest
  CYANREX_BENCH_BROADCAST_BUFFER=1024
  CYANREX_BENCH_JSON=0
  CYANREX_BENCH_JSON_FILE=<path>
  CYANREX_BENCH_LABEL=...
  CYANREX_BENCH_VERIFY=1
  CYANREX_BENCH_VERIFY_TIMEOUT=30
  CYANREX_BENCH_DATABASE_URL=postgres://...
EOF
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
fi

echo "[bench] events=${EVENTS} users=${USERS} concurrency=${CONCURRENCY} payload=${PAYLOAD_SIZE} policy=${POLICY}"

CMD=(cargo run --manifest-path "$ROOT_DIR/engine/Cargo.toml" --bin event_bus_bench -- \
  --events "$EVENTS" --users "$USERS" --concurrency "$CONCURRENCY" --payload-size "$PAYLOAD_SIZE" \
  --max-records "$MAX_RECORDS" --policy "$POLICY" --broadcast-buffer "$BROADCAST_BUFFER" \
  --verify-timeout "$VERIFY_TIMEOUT")

if [ -n "$DATABASE_URL" ]; then
  CMD+=(--database-url "$DATABASE_URL")
fi

if [ "$VERIFY" = "1" ]; then
  CMD+=(--verify)
fi
if [ "$OUTPUT_JSON" = "1" ]; then
  CMD+=(--json)
fi
if [ -n "$LABEL" ]; then
  CMD+=(--label "$LABEL")
fi

if [ "$OUTPUT_JSON" = "1" ]; then
  if [ -n "$OUTPUT_JSON_FILE" ]; then
    TMP_OUTPUT="$(mktemp)"
    trap 'rm -f "$TMP_OUTPUT"' EXIT
    "${CMD[@]}" | tee "$TMP_OUTPUT"
    tail -n 1 "$TMP_OUTPUT" >> "$OUTPUT_JSON_FILE"
  else
    exec "${CMD[@]}"
  fi
else
  exec "${CMD[@]}"
fi
