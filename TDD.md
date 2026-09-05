# TDD Policy

This repository now follows Test-Driven Development by default.

## Required cycle

1. Write or update a failing test first (Red).
2. Implement the minimum code to pass (Green).
3. Refactor while keeping tests green (Refactor).

## Minimum rule for backend route changes

- Every new route must include at least one success-case test.
- Behavior change requires a regression test.

## Command

```bash
cargo test --manifest-path engine/Cargo.toml
```

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
