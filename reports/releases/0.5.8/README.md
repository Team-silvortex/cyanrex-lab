# 0.5.8 source version validation

Date: **2026-10-13**. Previous release: **v0.5.7**. This compatible patch connects exact recipient
metadata, ordered private chunk download, original assembly and explicit host-selected saving.
Owned-directory targets, local progress and cooperative cancellation retain real-job resources.
One Source/chunk workspace now configures paired unmounted HTTP endpoints with independent budgets.
The normal application remains unmounted for distribution; Session issuance, privilege isolation,
UI/SDK saving, migration, resume and peer pairing are not introduced.

## Fresh release gate

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.5.8** after
the narrowly scoped test synchronization correction below:

| Selection | Actual final result |
|---|---|
| Rust defaults and documentation | **1,097 passed**, **858 opt-in ignored**, **98 successful result groups**, zero Rust warnings/failures; includes **15 compile-fail doctests** |
| Common checks | **351 passed**; file limits, version/course synchronization, generated OpenAPI/SDK/agent contracts, compatibility and management-tool checks |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **208 tests passed** |
| SDK | Build/runtime/types and package-consumer checks passed; **39 runtime + 3 package tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency-version changes |

The frontend test commands emitted **21 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings**
under Node.js 24.21.0. These are retained in the log, not counted as test failures or hidden by an
environment override; changing the frontend's module policy is outside this file-distribution patch.
No other warning headers were observed in the successful final gate.

Installed frontend/SDK dependency trees were checked before reusing them. Only root package versions
changed in the npm/Cargo locks; dependency entries stayed unchanged. Inherited database, fallback,
PostgreSQL, dotenv, Node and native-release-tool injection settings were removed for the gate.
No deployed database/configuration/service or privileged live-kernel workflow was used.

## Initial failure and test correction

The first release gate stopped in Rust library tests with **379 passed, 1 failed, 114 ignored**.
`cancelled_observed_chunk_eof_keeps_real_queued_job_but_never_claims_unobserved_ack_prefix` assumed
that restored whole-flow admission also meant the enclosing staging worker had released its permit.
The leased reader can drop its flow reference before the blocking closure drops the worker permit;
the immediate real verifier therefore legitimately returned `CapacityExhausted`.

Only that private test changed: after flow restoration it uses a ten-second bounded actual read-only
verifier, yielding only on temporary capacity saturation and returning every other result unchanged.
The successful verification, original bytes/hash/mode, unchanged observation, no-next-request and
no-selected-output assertions remain strict. Production admission, release order and cancellation
are unchanged. The focused case passed, then **20 consecutive reruns passed** without warnings;
the entire release gate was rerun from the beginning and passed. The initial failure log is retained
alongside the focused, repeated and final gate logs.

## Release identity and retained evidence

Canonical Engine, root Cargo/npm locks, frontend/SDK packages, OpenAPI info, README and runner-agent
documents are synchronized to **0.5.8**. API compatibility stays against frozen **0.3.0**; public
OpenAPI operations and SDK surfaces are unchanged. The ten Unreleased entries moved into the dated
0.5.8 changelog section. Bilingual source-inclusion notes and course copies distinguish inclusion
from earlier implementation results; this patch does not rescore the capability tensor.

The preceding [distribution-router verification](../../../docs/en/large-file-distribution-router.md)
retains its **0.5.7** implementation scope, including 19 explicit SQL and 3 controlled HTTPS cases.
Those cases are not fresh 0.5.8 SQL/TLS release acceptance, and opt-in ignored cases are not passes.
All **106 historical status blocks**, **56 existing execution tails**, the capability tensor and
earlier release reports remain intact. The five protected version metadata files were intentionally
advanced; other protected evidence stayed unchanged. The related-source guard confirmed **82 of
83 files unchanged**, with only the private progress test correction described above differing.

There was no fresh SQL/TLS/browser suite, RustSec audit, current LAN/SSH deployment, live-kernel
acceptance, local offline archive build/extraction/smoke, disk-full/fsync/power-loss test or throughput/RSS
benchmark for this release. The fresh npm production audits are separate from RustSec and deployment.

Raw release logs remain outside the repository at `/tmp/cyanrex-release-0.5.8-LYdY54Bc`.
This record precedes the version commit. Clean-candidate and annotated-tag preflight must bind
`v0.5.8` to the exact version commit before branch/tag publication; existing tags are never moved.
The tag workflow separately builds and validates an offline candidate; tag publication alone is not
a successful candidate result, a GitHub Release or deployment acceptance.
