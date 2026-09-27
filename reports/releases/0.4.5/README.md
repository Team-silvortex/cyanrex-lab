# 0.4.5 source-version validation

Date: 2026-09-27. Implementation baseline: `e0e0ebc3cbcbcaa947ec4b6edc692603ab62be80`.
Previous release: `v0.4.4`. This patch includes C1-C attributed policy commands/audit and C1-D
attributed identity binding/retirement, transactional receipts, replay and last-manager protection.
It also fixes absent-key identity reads racing an unfinished binding. These are explicit staging
stores, not live authentication, session revocation, startup migrations or protected-operation guards.

The security-enabled full quality gate passed in a disposable copy after synchronizing version 0.4.5:

- 331 default Rust tests passed; 142 external integration/benchmark cases were default-ignored.
- 62 identity/access/policy-audit/lifecycle-audit PostgreSQL cases were explicitly run on a new
  private-socket PostgreSQL 16 cluster, with no TCP listener. All passed; the four closed-pool
  default cases also passed again. The database contained only synthetic test accounts and schemas.
- The real loopback Runner Agent registration/probe/Clang test was explicitly run and passed.
  This is compiler/protocol acceptance, not TLS/LAN or kernel-loading acceptance.
- 75 common checks, 125 frontend checks, 18 generated production pages, frontend types,
  13 SDK runtime tests, SDK types and 3 package-consumer checks passed.
- RustSec and both production npm audits reported zero vulnerabilities. No exceptions were added.

Commands: `./scripts/quality-gate.sh --security`, followed by `cargo test --manifest-path
engine/Cargo.toml --locked --test collaboration_access_store_tdd --test
collaboration_identity_store_tdd --test collaboration_policy_audit_tdd --test
collaboration_identity_audit_tdd --test runner_agent_client_tdd -- --include-ignored --test-threads=4`
with only the disposable test database selected. Application database settings were not inherited.
The validation copy used separate build outputs, freshly installed locked npm dependencies and
disposable runtime data; existing Next/Engine services and their build directories were not modified.

[validation.json](validation.json) records counts and the input fingerprint. All 1,162 source inputs
matched the validation copy byte-for-byte and by Git-compatible executable/symlink mode before and
after the checks. The new release record itself is excluded to avoid a circular hash. The manifest
hashes newline-terminated compact JSON objects `{path,mode,sha256}` in JavaScript default sorted path
order; symlink payloads are their link text, not dereferenced content. Inputs are
`git ls-files -co --exclude-standard -z`, deduplicated, excluding `reports/releases/0.4.5/`.

Historical reports, released migration templates and frozen API/network evidence are unchanged.
The remaining 79 default-ignored cases, browser regressions, SSH/LAN enrollment, live-kernel loading,
database backup restoration and offline distribution artifact acceptance were not rerun. Temporary
builds and synthetic data can be recreated from the same inputs and commands.

This record precedes the version commit; candidate/annotated-tag preflight is required afterward.
No remote CI or publication is claimed. Source-version validation does not establish deployment or
offline-artifact acceptance. Publishing the branch/tag remains an explicit separate operation.
