# 0.4.2 source-version validation

Date: 2026-09-24. Previous commit: `f56349b4412cb72d2330a7fae11ad597cf6e82a1` (0.4.1).
This patch includes functional-network passes 13–15: settings verification/partial saves, metrics
lifecycle/isolation, and Runner inventory/probe/cancellation safety. Version metadata, package roots,
OpenAPI metadata, current examples and bilingual status documents are synchronized to 0.4.2.

This record is prepared before the version commit. It documents source checks, not an offline artifact,
image, deployment, tag publication or live system acceptance. Candidate and annotated-tag preflight are
separate post-commit checks; no push or deployment is part of this request.

| Check | Result |
|---|---|
| Full local quality gate | Pass; 310 Rust tests, 80 default-ignored integrations/benchmarks; 69 common tests, 125 frontend tests; SDK 13 runtime and 3 package tests plus types. |
| Build/contracts | Type checks and 18 production routes; OpenAPI/SDK compatibility remains intact; frontend and SDK production dependency audits report zero vulnerabilities. |
| Browser regressions | 56 passed: 17 settings, 15 metrics and 24 Runner cases, actual React components with controlled responses/clock. |
| Production-page subset | 2 passed: teacher navigation and full-job-ID cancellation/settings review, loopback Next server with synthetic intercepted APIs. |
| Final preflight | Metadata, mirrors, lengths, common tooling/tests, contracts and Rust formatting passed. |
| Frozen network | Expected exit 1: 0.3.8 → 0.4.2 and 26 cumulative source drifts; no access/API/page/catalog-count drift. |

`validation.json` records scope and limitations. `log-manifest.json` records exact raw log bytes.
`source-inputs.json` identifies all 131 pending files held unchanged through validation; newly authored
release evidence is separate. `frozen-receipt.json` proves 386 earlier reports/logs/numbered documents,
course mirrors and frozen inventory files are byte-identical, retaining their original versions/hashes.

Validation uses Node 24.19.0, installed Chrome 153.0.8010.52 and the existing locked dependencies.
Cold Rust compilation uses two jobs and the repository's small-cache development profile. Database
environment variables are unset. No extra dependency installation, live PostgreSQL, real Agent,
SSH/LAN enrollment, eBPF loading, offline artifact acceptance or remote CI run is claimed. RustSec
was not rerun; the full gate's frontend/SDK production audits were run.

The owned loopback preview is stopped. Generated Rust test outputs are removed with `cargo clean`
and can be rebuilt. Generated frontend/type/SDK caches are moved to desktop trash and remain
recoverable; trash movement is not counted as freed disk space. Runtime data, dependencies and all
historical evidence are preserved.
