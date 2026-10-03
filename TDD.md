# TDD Policy

This repository now follows Test-Driven Development by default.

It covers the existing teaching runtime, general collaboration preparation layers and task-owned
editors. Start with the [current testing guide](docs/en/testing-guide.md) for the complete matrix;
the [historical test network](docs/en/testing-network.md) records a past run, not today's full suite.

## Required cycle

1. Write or update a failing test first (Red).
2. Implement the minimum code to pass (Green).
3. Refactor while keeping tests green (Refactor).

## Minimum rule for backend route changes

- Every new route must include at least one success-case test.
- Behavior change requires a regression test.

## Service and cross-module changes

- Test generic contracts without assuming a teaching role, code payload or Run.
- For persistent commands, cover ownership/scope, stale revision and no-op rejection, atomic events,
  post-write verification and uncertain commit outcomes on disposable storage.
- Session commands must cover revocation, expiry, account incarnation and wait/cancellation boundaries;
  a success path through a trusted direct store does not validate authorization.
- For local editors, cover parent-owned state, obsolete callbacks, confirmation targets and rejected
  changes. Browser fixtures must state which Engine calls are mocked and whether real workers are used.
- Preserve existing teaching and SDK contracts when adding a general capability. Add explicit CI test
  selection for opt-in boundaries, including a check that each selected test name exists.

## Command

```bash
cargo test --manifest-path engine/Cargo.toml --locked
```

This is the default Rust suite, not all PostgreSQL, browser, SDK or privileged kernel acceptance.
Use `./scripts/quality-gate.sh` for the broader default gate and the testing guide for separately enabled suites.

## Optional learning integration checks

Use a disposable PostgreSQL database for the migration/concurrency test. It creates and removes its own
random schema; do not point it at a production database.

```bash
CYANREX_TEST_DATABASE_URL=postgresql://USER@localhost/TEST_DATABASE \
  cargo test --manifest-path engine/Cargo.toml --lib postgres_feedback -- --ignored
```

For the teacher/student browser workflow, build the frontend and serve it locally on port 3217, then
run `npm --prefix frontend run test:learning-browser`. The optional test requires an installed Playwright
runtime and Chromium. `CYANREX_PLAYWRIGHT_MODULE` can point to an existing Playwright module and
`CYANREX_CHROMIUM_PATH` to an existing browser binary. Override `CYANREX_UI_BASE_URL` for the frontend
address and `CYANREX_UI_ENGINE_URL` to match its configured Engine URL. All Engine calls are mocked;
this checks UI behavior, not live authentication or kernel execution. Backend route tests cover authorization,
CSRF, ownership, persistence errors, and revision conflicts separately.
