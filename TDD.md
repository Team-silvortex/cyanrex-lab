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
