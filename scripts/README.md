# scripts

Utility scripts for Cyanrex local operation.

- `check-instance-conflicts.sh`: preflight checker for multi-instance launches:
  - verifies requested engine/frontend/Postgres ports are free
  - detects whether a compose project with the same instance ID is already running
  - supports `--allow-existing-running` for re-attach flows
  - supports concurrent launch safety by preflighting duplicate ports and project IDs
- `start-lock.sh`: shared runtime helper loaded by `start.sh` for same-instance
  compose-operations mutual exclusion (start/stop/status/logs).
- `quality-gate.sh`: single-command quality gate for submit/CI checks.
  - default: run file-length + backend + frontend checks
  - `--backend-only`: run file-length + backend checks
  - `--frontend-only`: run file-length + frontend checks
  - `--format-only`: run file-length + Rust format check
  - `--permissions-only`: run file-length + permission regression checks
  - `--security`: run file-length + security audit check in addition to backend/frontend checks
  - `--security-only`: run only security audit check
  - `--no-npm-install`: skip `npm ci` during frontend checks
- `check-security-audit.sh`: audit Rust dependencies against a tracked exception registry.
  - reads `scripts/security-audit-exceptions.json`
  - reports explicit exceptions and fails on unapproved advisories

- `bench-event-bus.sh`: run the local event-bus throughput benchmark.
  - configurable through environment variables:
    - `CYANREX_BENCH_EVENTS`
    - `CYANREX_BENCH_USERS`
    - `CYANREX_BENCH_CONCURRENCY`
    - `CYANREX_BENCH_PAYLOAD_SIZE`
    - `CYANREX_BENCH_MAX_RECORDS`
    - `CYANREX_BENCH_POLICY=drop_oldest|drop_new`
    - `CYANREX_BENCH_BROADCAST_BUFFER`
    - `CYANREX_BENCH_VERIFY` (`1` enables)
    - `CYANREX_BENCH_VERIFY_TIMEOUT`
    - `CYANREX_BENCH_DATABASE_URL`
    - `CYANREX_BENCH_JSON` (`1` to include JSON output)
    - `CYANREX_BENCH_JSON_FILE` (append the JSON result line to a file)
    - `CYANREX_BENCH_LABEL` (optional benchmark label)
  - JSON result includes:
    - `throughput`
    - `avg_publish_latency_ms`
    - `p50_publish_latency_ms`
    - `p95_publish_latency_ms`
    - `p99_publish_latency_ms`
    - `max_publish_latency_ms`

- `bench-event-bus-compare.sh`: compare two named benchmark scenarios (`A` and `B`) and print a delta summary.
  - configure each scenario via `CYANREX_BENCH_<KEY>_A` and `CYANREX_BENCH_<KEY>_B`
    (with common `CYANREX_BENCH_<KEY>` fallbacks)
  - supported keys: `EVENTS`, `USERS`, `CONCURRENCY`, `PAYLOAD_SIZE`,
    `MAX_RECORDS`, `POLICY`, `BROADCAST_BUFFER`, `VERIFY_TIMEOUT`, `DATABASE_URL`
  - output default: `scripts/bench-event-bus-compare-<timestamp>.jsonl`
- `perf-regression-check.sh`: run a benchmark in JSON mode, append result to JSONL history,
  and optionally compare against a baseline JSON with threshold checks.
  - outputs throughput/p50/p95/p99 latency deltas
  - supports CI-friendly fail-fast thresholds through environment variables:
    - `CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT` (default `-5`, minimum throughput delta)
    - `CYANREX_BENCH_MAX_P50_INCREASE_PCT` (default `15`)
    - `CYANREX_BENCH_MAX_P95_INCREASE_PCT` (default `20`)
    - `CYANREX_BENCH_MAX_P99_INCREASE_PCT` (default `30`)
    - `CYANREX_BENCH_BASELINE_JSON` (optional path, single-line JSON baseline)
- `perf-baseline-capture.sh`: generate/update a single-line baseline JSON in one command.
  - default output: `scripts/perf-baseline/event-bus-baseline.json`
  - command:
    - `./scripts/perf-baseline-capture.sh`
  - run this after the system reaches steady-state baseline conditions and commit the output file.
- `perf-baseline/perf-thresholds.env`: baseline path and threshold presets for local regression runs.

Common workflow:

```bash
# 1) generate baseline
./scripts/perf-baseline-capture.sh

# 2) run regression (with defaults)
set -a; source scripts/perf-baseline/perf-thresholds.env; set +a
./scripts/perf-regression-check.sh

# 3) run with stricter thresholds
CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT=-10 \
CYANREX_BENCH_MAX_P99_INCREASE_PCT=40 \
./scripts/perf-regression-check.sh
```

In CI, workflow `Performance Regression` can also consume a baseline by passing `baseline_json`.
- `package-distribution.sh`: build and package distributable artifacts for air-gapped deployment.
  - builds and exports engine/frontend Docker images
  - generates one-command `run.sh` and `stop.sh`
  - outputs `dist/cyanrex-lab-<version>-<timestamp>.tar.gz` by default

Run it explicitly for a quick classroom readiness check:

```bash
./scripts/check-instance-conflicts.sh \
  --instance-id room-a \
  --engine-port 18080 \
  --frontend-port 13000 \
  --postgres-port 15433
```

Run quality checks before commit/PR:

```bash
./scripts/quality-gate.sh --format-only   # lightweight format + file-length check
./scripts/quality-gate.sh --backend-only   # backend + file-length check
./scripts/quality-gate.sh --frontend-only --no-npm-install   # frontend + file-length check
./scripts/quality-gate.sh --security-only  # security audit only
./scripts/quality-gate.sh --permissions-only  # permission regression checks only
./scripts/quality-gate.sh --security       # adds security audit to full checks
./scripts/quality-gate.sh                  # full checks
```

Run security audit directly:

```bash
./scripts/check-security-audit.sh
```

Build a distribution package:

```bash
./scripts/package-distribution.sh --version 0.1.0
```

If you already have local images (for example CI or private registry preloads), package without rebuilding:

```bash
./scripts/package-distribution.sh --skip-build --engine-image myrepo/cyanrex-engine:0.1.0 --frontend-image myrepo/cyanrex-frontend:0.1.0
```
```
