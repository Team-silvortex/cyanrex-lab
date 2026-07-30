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
  - `--no-npm-install`: skip `npm ci` during frontend checks

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
./scripts/quality-gate.sh                  # full checks
```
