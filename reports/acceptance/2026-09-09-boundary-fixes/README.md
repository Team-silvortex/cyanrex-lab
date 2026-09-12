# Boundary fix verification — 2026-09-09

All **nine** findings from the preceding module/boundary sweep are fixed; the original regression
assertions pass. This checks uncommitted **0.3.7** source, not a release artifact or the running deployment.
The earlier [failed report](../2026-09-09-test-network/result.json) remains unchanged.

## Fixes

| Area | Findings closed | Change |
|---|---:|---|
| Authentication | 4 | Confirm session insertion and verified credentials transactionally before publication; reject surviving sessions before account-delete commit |
| Local scripts | 3 | Preserve corrupt/failed snapshots; serialize load, private temporary file, atomic rename and memory publication, including admitted cancellation |
| Event deletion | 2 | Remove matches, reject invalid filters, preserve unread state, and never rewrite SQL history from a bounded cache |

Added **15 Rust regressions** for stale memory credentials, SQL login cancellation/password races,
script concurrency/cancellation/permissions/owner validation, and event cancellation/publication/unread/
partial-SQL-cache boundaries. Existing event invalid-filter coverage also checks empty values,
unknown keys, negative/overflowing windows and reversed ranges. Two Node cases guard API failure
contracts and CI selection. No original assertion was weakened or moved to an ignored test.

## Verification

| Layer | Result |
|---|---|
| Portable Rust | 263 passed; 29 opt-in cases excluded from the default gate |
| Real disposable PostgreSQL | 25 passed: auth 22, scripts 1, learning 1, events 1 |
| Auth race repeat | All 10 boundary cases also passed serially; not counted twice |
| Real signed Agent HTTP / Clang | 1 passed; compilation only, no kernel load |
| Real WebSocket | Paced, burst and stalled scenarios passed; one distinct test |
| Rust total | **290 distinct passed / 292 inventoried**; only two performance-only cases not run |
| Frontend unit / production-browser | 44 / 35 passed; browser Engine responses mocked |
| Tooling / SDK runtime and package | 55 / 16 passed |
| Build, types, formatting, contracts, docs, audits | Full quality gate including security passed |

The [plan](plan.json) assigns every Rust test to one module; [result](result.json) lists commands,
resolved finding IDs, scope limits and cleanup. [Source inputs](source-inputs.json) binds 399 maintained
inputs, including new modules, to hashes. It is not a whole-worktree or release-artifact manifest.

Run the normal gate with real database variables unset and a separate test data directory:

```bash
env -u DATABASE_URL -u CYANREX_TEST_DATABASE_URL ./scripts/quality-gate.sh --no-npm-install --security
```

Use Node 22 or newer, installed locked dependencies, and a fresh `CYANREX_DATA_DIR`. For opt-in SQL
tests, provision a **new disposable database** and set only `CYANREX_TEST_DATABASE_URL` plus
`CYANREX_DB_FALLBACK=true`. Never point fault-injection tests at classroom data. Exact test selections
and zero-selection guards are in the Engine CI steps. Evidence commands replace machine paths and
disposable credentials with placeholders; resolve them locally instead of executing placeholders.

## Scope and cleanup

No commit, push, installation, existing-service restart or host-kernel program load was performed.
The synthetic PostgreSQL container and test data were removed, the temporary frontend stopped,
and the tracked TypeScript build cache restored. Existing Engine/frontend/PostgreSQL start times were
unchanged; Engine health and frontend login returned HTTP 200.

Real LAN/TLS teacher-student onboarding, current-source eBPF kernel acceptance and actual SSH
deployment remain separate work. Async event queue/restart/failover, cross-process script writers and
power-loss durability are not established by these fixes. The existing volatile auth fallback remains
limited; cancellation of an already-dispatched enrollment may leave an account but never reopens its
consumed invitation. GitHub Actions was not executed in this local repair run.
