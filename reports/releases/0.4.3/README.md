# 0.4.3 source-version validation

Date: 2026-09-25. Baseline: `bea50cde4a4cbe087dff9a602f1c7e03970ed946` (0.4.2).
This patch updates rustls 0.23.43 → 0.23.45 and rustls-webpki 0.103.13 → 0.103.15,
fixing [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html).
An offline dependency-floor regression prevents rollback; no audit exception was added.
Current package metadata, OpenAPI, examples and bilingual documents are synchronized to 0.4.3.

The security-enabled full quality gate passed: 310 Rust tests (80 default-ignored external
integrations/manual benchmarks), 71 common checks, 125 frontend checks, SDK 13 runtime and
3 package checks plus types, and 18 production routes. RustSec and both production npm audits
reported zero vulnerabilities. One ignored Runner Agent test was then explicitly run and passed:
real loopback signed registration/probe/Clang compilation, not TLS/LAN or kernel loading.

`validation.json` records exact source-input and raw log hashes. All 397 earlier tracked reports,
numbered bug-hunt documents/mirrors and frozen network maps retain their original bytes and baselines.
Raw logs deliberately retain terminal carriage returns and final blank lines; source whitespace checks
exclude only these two generated transcripts so their recorded byte hashes remain reproducible.
This record is prepared before the version commit; candidate/tag checks and the authorized main/v0.4.3
push occur afterwards. Earlier local v0.4.2 remains unchanged; changelog comparisons use its exact commit
so they do not depend on publishing that older tag. Remote CI results are not claimed in this record.

PostgreSQL-specific integrations, browser regressions, SSH/LAN enrollment, live-kernel loading and
local offline-artifact acceptance were not rerun. No running deployment, credentials or data changed.
Rebuild and redeploy Engine/Runner Agent binaries or images to apply the TLS fix.

The owned Rust test cache was cleaned (1971 files, reported 2.3 GiB) and can be rebuilt. Frontend/SDK
build outputs were retained; existing Next servers were left alone. Dependencies, runtime data,
release history and historical evidence are preserved.
