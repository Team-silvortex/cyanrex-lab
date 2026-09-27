# 0.4.4 source-version validation

Date: 2026-09-27. Implementation baseline: `feec8494189b6ee22f20bbd8737fe190df3b22f3`.
Previous release: `v0.4.3`. This patch includes C0/C-M1 typed collaboration contracts,
C1-A durable identity staging and C1-B revision-checked membership/deployment policy staging.
It does not connect these stores to live authentication, startup migrations or protected operations.

The security-enabled full quality gate passed in a disposable copy of the working tree:

- 329 default Rust tests passed; 110 external integration/benchmark cases were default-ignored.
- 30 identity/access PostgreSQL cases were explicitly run on a new private-socket PostgreSQL 16
  cluster, with no TCP listener. All passed; both closed-pool default cases also passed again.
- The real loopback Runner Agent registration/probe/Clang test was explicitly run and passed.
  This is compiler/protocol acceptance, not TLS/LAN or kernel-loading acceptance.
- 73 common checks, 125 frontend checks, 18 generated production pages, frontend types,
  13 SDK runtime tests, SDK types and 3 package-consumer checks passed.
- RustSec and both production npm audits reported zero vulnerabilities. No exceptions were added.

Commands: `./scripts/quality-gate.sh --security`, followed by `cargo test --manifest-path
engine/Cargo.toml --locked --test collaboration_access_store_tdd --test
collaboration_identity_store_tdd --test runner_agent_client_tdd -- --include-ignored` with only
the disposable test database selected. Application database settings were not inherited.

[validation.json](validation.json) records counts and the input fingerprint. All 1,134 source inputs
matched the validation copy byte-for-byte and by Git-compatible executable/symlink mode. The new
release record itself is excluded to avoid a circular hash. The manifest hashes newline-terminated
compact JSON objects `{path,mode,sha256}` in JavaScript default sorted path order; symlink payloads
are their link text, not dereferenced content. Inputs are `git ls-files -co --exclude-standard -z`,
deduplicated, excluding `reports/releases/0.4.4/`.

Historical reports and frozen API/network evidence are unchanged. The remaining 79 default-ignored
cases, browser regressions, SSH/LAN enrollment, live-kernel loading, database backup restoration and
offline distribution artifact acceptance were not rerun. Existing Next/Engine services and runtime
data were left alone; generated frontend/SDK outputs and synthetic databases stayed in the temporary
validation directory. Temporary generated data can be recreated from the same commands.

This record precedes the version commit; candidate/annotated-tag preflight follows the commit.
No remote CI or publication is claimed. Source-version validation does not establish deployment or
offline-artifact acceptance. Publishing the branch/tag remains an explicit separate operation.
