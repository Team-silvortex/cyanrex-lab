# 0.4.6 source-version validation

Date: 2026-09-27. Implementation baseline: `c01b703f318720a9644b2c179fdc3cade1d4215c`.
Previous release: `v0.4.5`. This patch includes C1-E explicit empty-source durable authentication
and C1-F session-authorized identity binding/policy transactions, with exact account incarnations,
transactional audit, revocation ordering and source-backed last-manager protection. It rejects
incompatible empty authentication schemas and missing/recreated fallback-manager accounts.
These are internal staging adapters, not live authentication cutover, automatic migration or deployment.

The security-enabled full quality gate passed in a disposable copy after synchronizing version 0.4.6:

- 334 default Rust tests passed; 172 external integration/benchmark cases were default-ignored.
- 114 PostgreSQL cases passed on a new private-socket PostgreSQL 16 cluster with no TCP listener:
  62 prior identity/access/audit cases, 14 durable-source cases, 16 session-command cases and 22
  legacy authentication cases. The six default closed-pool cases also passed again. All database
  accounts and schemas were synthetic; no application database settings were inherited.
- The real loopback Runner Agent registration/probe/Clang test passed. This is compiler/protocol
  acceptance, not TLS/LAN or kernel-loading acceptance.
- 77 common checks, 125 frontend checks, 18 generated production pages, frontend types,
  13 SDK runtime tests, SDK types and 3 package-consumer checks passed.
- RustSec and both production npm audits reported zero vulnerabilities. No exceptions were added.

Commands, using separate build outputs, fresh locked npm dependencies and disposable runtime data:

```bash
CARGO_BUILD_JOBS=2 ./scripts/quality-gate.sh --security
cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_identity_store_tdd --test collaboration_access_store_tdd \
  --test collaboration_policy_audit_tdd --test collaboration_identity_audit_tdd \
  --test durable_auth_source_tdd --test durable_collaboration_tdd \
  -- --include-ignored --test-threads=4
cargo test --manifest-path engine/Cargo.toml --locked --lib services::auth_service::postgres \
  -- --ignored --test-threads=4
cargo test --manifest-path engine/Cargo.toml --locked --test runner_agent_client_tdd \
  -- --ignored --nocapture
```

Only the two PostgreSQL commands received a disposable `CYANREX_TEST_DATABASE_URL` and
`CYANREX_DB_FALLBACK=true`; `DATABASE_URL` and inherited application configuration were removed.
Existing Next/Engine services, their build directories and runtime data were not modified.

[validation.json](validation.json) records counts and the input fingerprint. All 1,186 source inputs
matched the validation copy byte-for-byte and by Git-compatible executable/symlink mode before and
after the checks. The new release record itself is excluded to avoid a circular hash. The manifest
hashes newline-terminated compact JSON objects `{path,mode,sha256}` in JavaScript default sorted path
order; symlink payloads are their link text, not dereferenced content. Inputs are
`git ls-files -co --exclude-standard -z`, deduplicated, excluding `reports/releases/0.4.6/`.

Historical reports, released migration templates 0001–0009 and frozen API/network evidence are unchanged.
The remaining 57 default-ignored cases, browser regressions, SSH/LAN enrollment, live-kernel loading,
database backup restoration and offline distribution artifact acceptance were not rerun. Temporary
builds and synthetic data can be recreated from the same inputs and commands.

This record precedes the version commit; candidate/annotated-tag preflight is required afterward.
No remote CI or publication is claimed by this validation record. Source-version validation does not
establish deployment or offline-artifact acceptance. Branch/tag publication is a separate operation.
