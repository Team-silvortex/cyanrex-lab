#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

OUTPUT_FILE="${CYANREX_BENCH_BASELINE_OUTPUT:-$ROOT_DIR/scripts/perf-baseline/event-bus-baseline.json}"
LABEL="${CYANREX_BENCH_BASELINE_LABEL:-baseline}"

print_help() {
  cat <<'EOF_HELP'
Usage: ./scripts/perf-baseline-capture.sh [output-json]

Capture one benchmark result as the performance baseline.

Environment variables:
  CYANREX_BENCH_BASELINE_OUTPUT  output file (default: scripts/perf-baseline/event-bus-baseline.json)
  CYANREX_BENCH_BASELINE_LABEL   benchmark label (default: baseline)

If an argument is provided, it is used as output file and overrides CYANREX_BENCH_BASELINE_OUTPUT.

Benchmark env variables are inherited for this capture:
  CYANREX_BENCH_EVENTS
  CYANREX_BENCH_USERS
  CYANREX_BENCH_CONCURRENCY
  CYANREX_BENCH_PAYLOAD_SIZE
  CYANREX_BENCH_MAX_RECORDS
  CYANREX_BENCH_POLICY
  CYANREX_BENCH_BROADCAST_BUFFER
  CYANREX_BENCH_VERIFY_TIMEOUT
  CYANREX_BENCH_DATABASE_URL
EOF_HELP
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  print_help
  exit 0
fi

if [ "$#" -gt 1 ]; then
  echo "error: too many arguments"
  print_help
  exit 1
fi

if [ "$#" -eq 1 ]; then
  OUTPUT_FILE="$1"
fi

mkdir -p "$(dirname "$OUTPUT_FILE")"

TMP_OUTPUT="$(mktemp)"
trap 'rm -f "$TMP_OUTPUT"' EXIT

(
  export CYANREX_BENCH_VERIFY=0
  export CYANREX_BENCH_JSON=1
  export CYANREX_BENCH_LABEL="$LABEL"
  "$ROOT_DIR/scripts/bench-event-bus.sh"
) | tee "$TMP_OUTPUT"

BASELINE_JSON="$(tail -n 1 "$TMP_OUTPUT")"
echo "$BASELINE_JSON" > "$OUTPUT_FILE"

echo "[baseline] captured to $OUTPUT_FILE"
