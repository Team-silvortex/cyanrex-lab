# scripts

Utility scripts for Cyanrex local operation.

- `check-instance-conflicts.sh`: preflight checker for multi-instance launches:
  - verifies requested engine/frontend/Postgres ports are free
  - detects whether a compose project with the same instance ID is already running
  - supports `--allow-existing-running` for re-attach flows
  - supports concurrent launch safety by preflighting duplicate ports and project IDs
- `start-lock.sh`: shared runtime helper loaded by `start.sh` for same-instance
  compose-operations mutual exclusion (start/stop/status/logs).

Run it explicitly for a quick classroom readiness check:

```bash
./scripts/check-instance-conflicts.sh \
  --instance-id room-a \
  --engine-port 18080 \
  --frontend-port 13000 \
  --postgres-port 15433
```
