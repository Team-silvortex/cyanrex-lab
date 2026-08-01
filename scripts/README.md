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
