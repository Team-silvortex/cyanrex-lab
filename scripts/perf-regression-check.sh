#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

OUTPUT_DIR="${CYANREX_BENCH_OUTPUT_DIR:-$ROOT_DIR/.run/perf}"
OUTPUT_FILE="${CYANREX_BENCH_OUTPUT_FILE:-$OUTPUT_DIR/event-bus-bench-compare.jsonl}"
BASELINE_JSON_FILE="${CYANREX_BENCH_BASELINE_JSON:-}"
BASELINE_LABEL="${CYANREX_BENCH_BASELINE_LABEL:-baseline}"
CURRENT_LABEL="${CYANREX_BENCH_REGRESSION_LABEL:-current}"

THROUGHPUT_MIN_DELTA_PCT="${CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT:--5}"
P50_MAX_INCREASE_PCT="${CYANREX_BENCH_MAX_P50_INCREASE_PCT:-15}"
P95_MAX_INCREASE_PCT="${CYANREX_BENCH_MAX_P95_INCREASE_PCT:-20}"
P99_MAX_INCREASE_PCT="${CYANREX_BENCH_MAX_P99_INCREASE_PCT:-30}"

print_help() {
  cat <<'EOF_HELP'
Usage: ./scripts/perf-regression-check.sh

Run a JSON mode event bus benchmark and optionally fail if it regresses against a baseline JSON.

Environment variables:
  CYANREX_BENCH_OUTPUT_DIR                 directory for JSONL output (default: .run/perf)
  CYANREX_BENCH_OUTPUT_FILE                explicit output file
  CYANREX_BENCH_BASELINE_JSON              path to baseline JSON (single-line benchmark output)
  CYANREX_BENCH_BASELINE_LABEL             baseline label in output
  CYANREX_BENCH_REGRESSION_LABEL            run label in output

  CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT    minimum throughput delta % allowed (default: -5)
  CYANREX_BENCH_MAX_P50_INCREASE_PCT        max allowed p50 increase (default: 15)
  CYANREX_BENCH_MAX_P95_INCREASE_PCT        max allowed p95 increase (default: 20)
  CYANREX_BENCH_MAX_P99_INCREASE_PCT        max allowed p99 increase (default: 30)

Bench env overrides are also respected:
  CYANREX_BENCH_EVENTS, CYANREX_BENCH_USERS, CYANREX_BENCH_CONCURRENCY,
  CYANREX_BENCH_PAYLOAD_SIZE, CYANREX_BENCH_MAX_RECORDS,
  CYANREX_BENCH_POLICY, CYANREX_BENCH_BROADCAST_BUFFER,
  CYANREX_BENCH_VERIFY_TIMEOUT, CYANREX_BENCH_DATABASE_URL
EOF_HELP
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  print_help
  exit 0
fi

mkdir -p "$OUTPUT_DIR"

TMP_OUTPUT="$(mktemp)"
trap 'rm -f "$TMP_OUTPUT"' EXIT

echo "[perf] running benchmark and writing JSON line"
(
  export CYANREX_BENCH_VERIFY=0
  export CYANREX_BENCH_JSON=1
  export CYANREX_BENCH_LABEL="$CURRENT_LABEL"
  "$ROOT_DIR/scripts/bench-event-bus.sh"
) | tee "$TMP_OUTPUT"

CURRENT_JSON="$(tail -n 1 "$TMP_OUTPUT")"
echo "$CURRENT_JSON" >> "$OUTPUT_FILE"
echo "[perf] current result appended to $OUTPUT_FILE"

if [ -z "$BASELINE_JSON_FILE" ]; then
  echo "[perf] no baseline configured; skipping regression assertions"
  exit 0
fi

if [ ! -f "$BASELINE_JSON_FILE" ]; then
  echo "[perf] baseline file not found: $BASELINE_JSON_FILE"
  exit 1
fi

BASELINE_JSON="$(tail -n 1 "$BASELINE_JSON_FILE")"

if ! command -v python3 >/dev/null 2>&1; then
  echo "[perf] python3 is required for regression checks. Install python3 and rerun."
  exit 1
fi

python3 - "$CURRENT_JSON" "$BASELINE_JSON" "$THROUGHPUT_MIN_DELTA_PCT" "$P50_MAX_INCREASE_PCT" "$P95_MAX_INCREASE_PCT" "$P99_MAX_INCREASE_PCT" "$BASELINE_LABEL" "$CURRENT_LABEL" <<'PY'
import json
import sys

current = json.loads(sys.argv[1])
base = json.loads(sys.argv[2])
min_throughput_delta = float(sys.argv[3])
p50_max = float(sys.argv[4])
p95_max = float(sys.argv[5])
p99_max = float(sys.argv[6])
base_label = sys.argv[7]
current_label = sys.argv[8]

def value(data, key):
    return float(data.get(key, 0.0))

def pct_delta(current_value, base_value):
    if base_value == 0:
        return 0.0
    return (current_value - base_value) / base_value * 100.0

throughput_current = value(current, 'throughput')
throughput_base = value(base, 'throughput')
p50_current = value(current, 'p50_publish_latency_ms')
p50_base = value(base, 'p50_publish_latency_ms')
p95_current = value(current, 'p95_publish_latency_ms')
p95_base = value(base, 'p95_publish_latency_ms')
p99_current = value(current, 'p99_publish_latency_ms')
p99_base = value(base, 'p99_publish_latency_ms')

throughput_delta = pct_delta(throughput_current, throughput_base)
p50_delta = pct_delta(p50_current, p50_base)
p95_delta = pct_delta(p95_current, p95_base)
p99_delta = pct_delta(p99_current, p99_base)

print(f"[perf] baseline: {base_label}")
print(f"{base_label} throughput: {throughput_base:.2f}, {current_label}: {throughput_current:.2f}, delta: {throughput_delta:+.2f}%")
print(f"{base_label} p50 latency: {p50_base:.3f}ms, {current_label}: {p50_current:.3f}ms, delta: {p50_delta:+.2f}%")
print(f"{base_label} p95 latency: {p95_base:.3f}ms, {current_label}: {p95_current:.3f}ms, delta: {p95_delta:+.2f}%")
print(f"{base_label} p99 latency: {p99_base:.3f}ms, {current_label}: {p99_current:.3f}ms, delta: {p99_delta:+.2f}%")

failed = False
if throughput_delta < min_throughput_delta:
    print(f"[perf][fail] throughput dropped by {throughput_delta:.2f}% (threshold: {min_throughput_delta:.2f}%)")
    failed = True
if p50_delta > p50_max:
    print(f"[perf][fail] p50 latency increased by {p50_delta:.2f}% (threshold: {p50_max:.2f}%)")
    failed = True
if p95_delta > p95_max:
    print(f"[perf][fail] p95 latency increased by {p95_delta:.2f}% (threshold: {p95_max:.2f}%)")
    failed = True
if p99_delta > p99_max:
    print(f"[perf][fail] p99 latency increased by {p99_delta:.2f}% (threshold: {p99_max:.2f}%)")
    failed = True

if failed:
    sys.exit(1)

print('[perf] regression checks passed')
PY
