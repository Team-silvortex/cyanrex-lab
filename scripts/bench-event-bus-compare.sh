#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_LABEL_A="${1:-}"
RUN_LABEL_B="${2:-}"
OUT_FILE="${CYANREX_BENCH_COMPARE_OUTPUT:-}"
if [ -z "$OUT_FILE" ]; then
  STAMP="$(date +%Y%m%d-%H%M%S)"
  OUT_FILE="${ROOT_DIR}/scripts/bench-event-bus-compare-${STAMP}.jsonl"
fi

if [ -z "$RUN_LABEL_A" ]; then
  RUN_LABEL_A="before"
fi
if [ -z "$RUN_LABEL_B" ]; then
  RUN_LABEL_B="after"
fi

usage() {
  cat <<'EOF'
Usage: ./scripts/bench-event-bus-compare.sh [label-a label-b]

Run two named benchmark phases and compare JSON results from each.

Set CYANREX_BENCH_*_... variables for each scenario:
  CYANREX_BENCH_EVENTS_A, CYANREX_BENCH_EVENTS_B
  CYANREX_BENCH_USERS_A, CYANREX_BENCH_USERS_B
  CYANREX_BENCH_CONCURRENCY_A, CYANREX_BENCH_CONCURRENCY_B
  CYANREX_BENCH_PAYLOAD_SIZE_A, CYANREX_BENCH_PAYLOAD_SIZE_B
  CYANREX_BENCH_MAX_RECORDS_A, CYANREX_BENCH_MAX_RECORDS_B
  CYANREX_BENCH_POLICY_A, CYANREX_BENCH_POLICY_B
  CYANREX_BENCH_BROADCAST_BUFFER_A, CYANREX_BENCH_BROADCAST_BUFFER_B
  CYANREX_BENCH_VERIFY_TIMEOUT_A, CYANREX_BENCH_VERIFY_TIMEOUT_B
  CYANREX_BENCH_DATABASE_URL_A, CYANREX_BENCH_DATABASE_URL_B

and common defaults with no suffix:
  CYANREX_BENCH_EVENTS, CYANREX_BENCH_USERS, ...

Output:
  ${CYANREX_BENCH_COMPARE_OUTPUT} (or generated .jsonl file) contains two JSON lines.
EOF
}

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

get_value() {
  local key="$1"
  local suffix="$2"
  local suffixed="CYANREX_BENCH_${key}_${suffix}"
  local base="CYANREX_BENCH_${key}"
  printf '%s' "${!suffixed:-${!base:-}}"
}

set_env_if_set() {
  local name="$1"
  local value="$2"
  if [ -n "$value" ]; then
    export "$name=$value"
  else
    unset "$name" || true
  fi
}

run_case() {
  local suffix="$1"
  local label="$2"

  local events
  local users
  local concurrency
  local payload_size
  local max_records
  local policy
  local broadcast_buffer
  local verify_timeout
  local database_url

  events="$(get_value "EVENTS" "$suffix")"
  users="$(get_value "USERS" "$suffix")"
  concurrency="$(get_value "CONCURRENCY" "$suffix")"
  payload_size="$(get_value "PAYLOAD_SIZE" "$suffix")"
  max_records="$(get_value "MAX_RECORDS" "$suffix")"
  policy="$(get_value "POLICY" "$suffix")"
  broadcast_buffer="$(get_value "BROADCAST_BUFFER" "$suffix")"
  verify_timeout="$(get_value "VERIFY_TIMEOUT" "$suffix")"
  database_url="$(get_value "DATABASE_URL" "$suffix")"

  local tmp
  tmp="$(mktemp)"
  {
    set_env_if_set CYANREX_BENCH_EVENTS "$events"
    set_env_if_set CYANREX_BENCH_USERS "$users"
    set_env_if_set CYANREX_BENCH_CONCURRENCY "$concurrency"
    set_env_if_set CYANREX_BENCH_PAYLOAD_SIZE "$payload_size"
    set_env_if_set CYANREX_BENCH_MAX_RECORDS "$max_records"
    set_env_if_set CYANREX_BENCH_POLICY "$policy"
    set_env_if_set CYANREX_BENCH_BROADCAST_BUFFER "$broadcast_buffer"
    set_env_if_set CYANREX_BENCH_VERIFY_TIMEOUT "$verify_timeout"
    set_env_if_set CYANREX_BENCH_DATABASE_URL "$database_url"
    export CYANREX_BENCH_VERIFY="0"
    export CYANREX_BENCH_JSON="1"
    export CYANREX_BENCH_LABEL="${label}"
    "$ROOT_DIR/scripts/bench-event-bus.sh"
  } > >(tee "$tmp" >&2)

  local json_line
  json_line="$(tail -n 1 "$tmp")"
  rm -f "$tmp"
  echo "$json_line" >> "$OUT_FILE"
  echo "$json_line"
}

echo "[compare] output: $OUT_FILE"
: > "$OUT_FILE"

json_a="$(run_case "A" "$RUN_LABEL_A")"
json_b="$(run_case "B" "$RUN_LABEL_B")"

if command -v python3 >/dev/null 2>&1; then
  python3 - "$json_a" "$json_b" <<'PY'
import json
import sys

first = json.loads(sys.argv[1])
second = json.loads(sys.argv[2])

def value(data, key, default=0.0):
    return float(data.get(key, default))

def delta(b, a):
    if a == 0:
        return 0.0
    return (b - a) / a * 100

print("[compare] throughput")
print(f"  {first['label']}: {value(first, 'throughput'):.2f}")
print(f"  {second['label']}: {value(second, 'throughput'):.2f}")
print(f"  delta: {delta(value(second, 'throughput'), value(first, 'throughput')):+.2f}%")

print("[compare] avg latency (ms)")
print(f"  {first['label']}: {value(first, 'avg_publish_latency_ms'):.3f}")
print(f"  {second['label']}: {value(second, 'avg_publish_latency_ms'):.3f}")
print(f"  delta: {delta(value(second, 'avg_publish_latency_ms'), value(first, 'avg_publish_latency_ms')):+.2f}%")

print("[compare] p50 latency (ms)")
print(f"  {first['label']}: {value(first, 'p50_publish_latency_ms'):.3f}")
print(f"  {second['label']}: {value(second, 'p50_publish_latency_ms'):.3f}")
print(f"  delta: {delta(value(second, 'p50_publish_latency_ms'), value(first, 'p50_publish_latency_ms')):+.2f}%")

print("[compare] p95 latency (ms)")
print(f"  {first['label']}: {value(first, 'p95_publish_latency_ms'):.3f}")
print(f"  {second['label']}: {value(second, 'p95_publish_latency_ms'):.3f}")
print(f"  delta: {delta(value(second, 'p95_publish_latency_ms'), value(first, 'p95_publish_latency_ms')):+.2f}%")

print("[compare] p99 latency (ms)")
print(f"  {first['label']}: {value(first, 'p99_publish_latency_ms'):.3f}")
print(f"  {second['label']}: {value(second, 'p99_publish_latency_ms'):.3f}")
print(f"  delta: {delta(value(second, 'p99_publish_latency_ms'), value(first, 'p99_publish_latency_ms')):+.2f}%")

print("[compare] max latency (ms)")
print(f"  {first['label']}: {value(first, 'max_publish_latency_ms'):.3f}")
print(f"  {second['label']}: {value(second, 'max_publish_latency_ms'):.3f}")
print(f"  delta: {delta(value(second, 'max_publish_latency_ms'), value(first, 'max_publish_latency_ms')):+.2f}%")
PY
else
  echo "[compare] Install python3 to get computed delta summary."
fi
