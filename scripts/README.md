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
  - every mode also checks file length, version/OpenAPI drift, and repository tooling
  - default: run backend + frontend + JavaScript SDK checks
  - `--backend-only`: run common preflight + backend checks
  - `--frontend-only`: run common preflight + frontend checks
  - `--sdk-only`: run common preflight + JavaScript SDK build, tests, and audit
  - `--format-only`: run common preflight + Rust format check
  - `--permissions-only`: run common preflight + permission regression checks
  - `--security`: add the security audit to the default backend/frontend/SDK checks
  - `--security-only`: run common preflight + security audit check
  - `--no-npm-install`: skip `npm ci` during frontend and SDK checks
- `check-security-audit.sh`: audit Rust dependencies against a tracked exception registry.
  - reads `scripts/security-audit-exceptions.json`
  - reports explicit exceptions and fails on unapproved advisories
- `check-version-sync.sh`: keeps the Engine, frontend, SDK, lockfiles, and release-facing docs on
  the same semantic version, including the generated OpenAPI document and current `CHANGELOG.md`
  release heading. `engine/Cargo.toml` is the canonical source.
- `release-preflight.mjs`: validates a clean committed candidate before tagging, or verifies an existing
  annotated version tag against every synchronized version field, dated changelog entry, and release doc.
- `tests/releasePreflight.test.mjs`: argument, drift-reporting, clean-tree, lightweight/annotated Tag,
  and immutable historical-tree release regressions.
- `release-metadata.mjs`: writes machine-readable offline-package provenance with Git source state,
  image references/content IDs/build mode, and streamed image-archive SHA-256; `--verify` checks an
  extracted package, while `--expect-*` flags bind a Tag candidate to its version, revision, source state,
  and image mode.
- `tests/releaseMetadata.test.mjs`: metadata validation, Git-state detection, path privacy, tamper, and
  checksum-binding regressions.
- `generate-openapi.mjs`: generates `engine/openapi/openapi.json` from the registered Axum routes,
  access rules, and maintained component schemas; use `--check` to reject stale output.
- `openapi-contract.mjs`: checks exact Engine/OpenAPI route and access-tier parity, expected Engine/SDK
  coverage, operation metadata, version sync, and schema references.
- `tests/openapiContract.test.mjs`: parser and drift-reporting regressions for the contract checks.
- `generate-sdk-types.mjs`: converts OpenAPI component schemas into the committed
  `sdk-js/src/generated/openapi.ts` type map; use `--check` to reject stale SDK models.
- `tests/sdkTypeGenerator.test.mjs`: JSON Schema-to-TypeScript rendering regressions.
- `generate-sdk-operations.mjs`: generates the 56 browser-facing SDK operation inputs, responses,
  access/transport metadata, and operationId registry while excluding the signed Runner Agent protocol.
- `tests/sdkOperationGenerator.test.mjs`: operation transport, parameter, response, and Agent-boundary
  generation regressions.
- `sdk-surface-compatibility.mjs`: checks the frozen additive-only `CyanrexClient` namespace/method
  baseline and rejects removed or renamed public member paths; `--write-baseline` is review-only.
- `tests/sdkSurfaceCompatibility.test.mjs`: nested surface extraction, removal, additive growth, and
  deterministic baseline regressions.
- `api-compatibility.mjs`: compares the current Engine contract with the frozen SDK baseline and
  rejects removed operations, access changes, narrowed inputs, or weakened successful responses.
  - `--write-baseline` intentionally replaces the baseline after an approved compatibility reset
  - normal quality checks never rewrite the baseline
- `tests/apiCompatibility.test.mjs`: breaking/additive request, response, access, enum, and nullability
  compatibility regressions.
- `clean-macos-metadata.mjs`: removes `.DS_Store` and AppleDouble `._*` sidecars that can be
  mistaken for source files by Next.js after copying the repository through a macOS filesystem.
- `frontend/scripts/sync-course-docs.mjs`: replaces the frontend's committed course copy from the
  authoritative `docs/` tree; `--check` rejects missing, changed, and unexpected mirrored files and is
  included in every quality-gate mode.
- `debug-system.sh`: collect environment/runtime diagnostics for local troubleshooting.
  - prints toolchain versions, kernel capability status, compose backend status, and port checks
  - intended for `./start.sh diagnose`
- `runner-agent.sh`: prepare and manage the optional unprivileged Compose compiler Agent.
  - `start`, `stop`, `status`, `logs`, and secret-only `prepare` actions
  - creates a missing bootstrap token without printing it and mounts a mode-0600 Docker Secret
- `runner-agent-smoke.sh`: authenticate, discover the managed Agent, submit a real compile check,
  poll its user-scoped result, and fail if the end-to-end remote diagnostics path is unhealthy.
- `test-runner-agent-tools.sh`: verify secret generation, mode 0600, stable reuse, non-disclosure,
  safe Agent IDs, and shell syntax without starting Docker.
- `live-kernel-smoke.sh`: on a disposable privileged Linux stack, authenticate, require an empty
  attachment set, run the built-in Aya `sched_switch` ring-buffer template, require a uniquely bound
  kernel event, detach its exact pin, and reject residue. `CYANREX_KERNEL_SMOKE_REPORT` optionally writes
  atomic evidence bound to packaged release metadata and the runtime environment.
- `live-kernel-evidence.py`: create and strictly verify versioned live-kernel evidence, including exact
  candidate metadata, image identities, environment, unique event, bpffs pin, and cleanup. The verifier
  accepts legacy v1 evidence while new reports use the self-contained v2 event binding.
- `test-live-kernel-smoke.sh`: mock successful, stale-event, and missing-event paths, validate generated
  and legacy evidence, reject tampering/metadata mismatches/duplicate keys, and require cleanup without
  loading a program into the local kernel.
- `release-candidate.py`: verify the exact four-file Tag artifact as one unit. It validates both outer
  checksums, streams the package without extraction, rejects unsafe or ambiguous archive structures,
  verifies every internal file, cross-binds release metadata to the live-kernel evidence, and optionally
  delegates verified extraction through `--extract-to`.
- `release-package.py`: verify an archive/checksum pair and safely extract it into a new, non-existing
  output directory. It writes regular files manually, rechecks each hash during extraction, sanitizes
  modes, and never asks the system `tar` command to interpret untrusted members.
- `test-release-candidate.sh`: build a minimal valid candidate and reject checksum tampering, evidence
  substitution, modified package members, traversal, symbolic links, ambiguous archives, and output
  replacement while exercising both candidate and package-only extraction paths.

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
  - builds or pulls and exports PostgreSQL, Engine, and frontend Docker images
  - generates `deploy.sh`, `run.sh`, `stop.sh`, and a disposable `install-smoke.sh`
  - emits checksum-bound `release-metadata.json` without build-host absolute paths
  - outputs `dist/cyanrex-lab-<version>-<timestamp>.tar.gz` by default
- `distribution-install-smoke.sh`: packaged as `install-smoke.sh`; validates a freshly extracted
  release, including checksums, packaged image content IDs, service health, frontend CSP, login, and
  remote Agent compilation; inherited host image overrides cannot replace candidate images. Set
  `CYANREX_SMOKE_RUN_LIVE_KERNEL=1` only on a disposable privileged Linux host to add live kernel
  attach/event/detach acceptance.
- `test-distribution-tools.sh`: static regression checks for packaging helpers and Compose runtime
  environment forwarding, plus non-publishing Tag candidate workflow invariants; included in every
  quality-gate mode.

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
./scripts/quality-gate.sh --sdk-only --no-npm-install  # SDK build/tests + file-length check
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
./scripts/package-distribution.sh --version 0.3.1
./scripts/package-distribution.sh --version 0.3.1 --compose-template docker/docker-compose.yml   # custom compose if needed
```

If you already have local images (for example CI or private registry preloads), package without rebuilding:

```bash
./scripts/package-distribution.sh --skip-build --engine-image myrepo/cyanrex-engine:0.3.1 --frontend-image myrepo/cyanrex-frontend:0.3.1
```

Packaging also honors `ENGINE_RUST_IMAGE`, `ENGINE_DEBIAN_IMAGE`, `ENGINE_APT_MIRROR`,
`ENGINE_CARGO_REGISTRY_INDEX`, `FRONTEND_NODE_IMAGE`, and `FRONTEND_NPM_REGISTRY`, matching the
source Compose build controls for restricted or mirrored networks.

Distributed package entry points:

```bash
./deploy.sh up     # start services
./deploy.sh status # check status
./deploy.sh logs   # tail logs
./deploy.sh down   # stop services
./runner-agent.sh start  # optional unprivileged compiler Agent
./runner-agent-smoke.sh  # authenticated remote compile smoke test
./live-kernel-smoke.sh   # privileged live attach/event/detach acceptance
python3 ./live-kernel-evidence.py verify /path/to/report.json --release-metadata ./release-metadata.json
./install-smoke.sh       # destructive disposable-host installation acceptance
# `run.sh` and `stop.sh` remain compatibility shortcuts.
```

Verify a complete downloaded Tag candidate before extraction (use a directory containing only the
archive, its checksum, the live-kernel report, and its checksum):

```bash
release_revision="$(git rev-list -n 1 v0.3.1)"
python3 scripts/release-candidate.py verify /path/to/downloaded-candidate \
  --expect-version 0.3.1 --expect-revision "$release_revision" --expect-tag v0.3.1 \
  --extract-to /path/to/new-output-directory
```

Safely extract a two-file package that has no live-kernel evidence:

```bash
python3 scripts/release-package.py extract /path/to/archive-and-checksum \
  --output /path/to/new-output-directory
```
