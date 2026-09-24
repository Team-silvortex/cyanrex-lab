# Chain-guided bug hunt 12: retention settings and overflow-policy confirmation

[简体中文](../zh-CN/functional-network-bug-hunt-12.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-12.json)

Date: 2026-09-24. Base: `main@bfecf77292bb87c01d69b8bbd3dc600de1f29e58`, version 0.4.0.
The worktree already contained uncommitted passes 09–11; this is not clean-release acceptance.
No version change, commit, tag, push or deployment. Earlier reports and the frozen inventory are preserved.

## Scope and findings

F36 settings → owner admission → publication barrier → policy/trim transaction → cache publication →
HTTP confirmation, including E59 history and E60 unread/overflow boundaries. Actual Axum handlers use
synthetic sessions and an explicitly disposable PostgreSQL 16.15 cluster on a private Unix socket.
`DATABASE_URL` is unset; no deployment data, real accounts, live Engine, SSH/LAN or kernel operations.

| ID | Evidence | Fix |
|---|---|---|
| BH12-01 | Settings reads returned `200` during a missing-table failure, prior fallback or an unknown stored policy. | Fresh fallible HTTP reads, distinct from the runtime cache. Missing rows remain valid defaults; storage/decoding/invalid stored-value failures return generic no-store `503`, not cached/default success. Read failure does not latch persistence or publish defaults. |
| BH12-02 | A failed publication barrier still produced `200`; a settings SQL error produced `400`, with the code forwarding storage error text. | Confirm configured storage and the barrier before policy/trim in one transaction, then publish memory. Unconfirmed writes return generic `503`; definite SQL errors preserve policy/history/unread and remain retryable. Prior fallback cannot silently save or trim memory. |
| BH12-03 | A save waiting for local cache publication exceeded the 12-second test bound. Successful GET also lacked no-store. | Reuse ten-second cooperative service waiting and admitted SQL worker ordering. Deadline/cancellation is not rollback. Success and handler errors are private; malformed JSON, oversized bodies, content type and field/type failures keep 400/413/415/422 with generic JSON. |

Legacy public settings helpers retain best-effort runtime caching/fallback. Both save paths share the
transaction and atomic local-publication helpers. Success shapes, 50..50000 request clamping, session
ownership, CSRF and SDK signatures are unchanged. Unknown body fields remain ignored, never owner overrides.
No database migration, new route/access tier or frontend product-source change is introduced.

## Failure-first evidence and coverage

Before implementation, six PostgreSQL cases and one HTTP case failed; contract/CI checks had two failures
among five cases. The initial HTTP guard failed at its first missing-header assertion. PostgreSQL guards
stop at their first failure: later corrupt-type/limit and repaired-path assertions are protective coverage,
not claims that every branch was separately reproduced before fixing. All ten final settings SQL cases pass.

Added **14 unique cases: three ordinary Rust, ten ignored PostgreSQL and one Node contract test**.
Four extra SQL cases cover bounded pool waits/read cancellation, admitted save cancellation with later
DropNew publication, a transaction blocked before commit, and fresh-bus reload/read flags/capacity rebasing.
Two service guards cover bad configuration versus memory mode and cancellation before cache/admission
publication. The HTTP test also covers request clamping, body failures, authentication and Origin checks.

| Check | Result | Boundary |
|---|---|---|
| Focused HTTP | 10 passed | One new and nine existing event/learning cases; overlaps the complete gate. |
| Default event/filter suite | 46 passed, 54 ignored | Ignored cases are 52 PostgreSQL and two manual benchmarks. |
| Settings PostgreSQL | 10 passed | Real SQL, synthetic owners, controlled failures, locks and cancellation. |
| All event PostgreSQL | 52 passed | Previous 42 plus ten new; separate explicit execution, not default passes. |
| All default Rust | 310 passed, 80 ignored | Complete default gate; explicit SQL results are counted separately. |
| Common, frontend and SDK gate | All passed | 69 common, 93 frontend, SDK 13 runtime/3 package plus type checks; 18 production routes; frontend/SDK production audits reported zero vulnerabilities. |
| Contract and CI selection | 7 passed | 51 ordering-job selections plus the separate deletion case; not a remote CI run. |
| Frozen map | Expected drift | 0.3.8 → 0.4.0 and 25 cumulative source changes; no access/API/page/catalog-count drift. |

The first full-gate launch failed before running checks because an unquoted PATH contained spaces;
its diagnostic is retained separately, and the corrected launch is not treated as a product fix.
All 288 historical inventory/report/log/mirror files remain byte-identical, including uncommitted passes
09–11. Source fingerprints identify selected tested files, not a clean repository snapshot. The gate uses
Node 24.19.0 and Rust/Cargo 1.95.0; source/report utilities also use host Node 24.21.0. Final documentation
and mirrors are rechecked separately without changing tested runtime sources. The private cluster had
zero test schemas and other clients before shutdown/removal. This pass's regenerated build caches were
also removed; source, dependencies, evidence and deployment remain intact. See the machine report receipts.

## Limits and next slice

- `503`, cancellation or response loss is **unconfirmed, not rollback**. One test deliberately holds cache
  publication after SQL commits, then verifies completion after the deadline. Another blocks DELETE
  before commit and observes rollback; it does not establish rollback for every timeout. Verify state
  before an explicit retry; no automatic retry was added.
- Ten seconds bounds cooperative service waiting, not auth/body extraction/serialization, CPU stalls or
  worker lifetime. An admitted SQL worker retains owner admission while awaiting cache locks. Existing
  writer/barrier/storage failures may latch fallback; repair/restart does not reconcile volatile events.
- HTTP reads do not update the runtime policy cache or establish coherence with external SQL writers.
  Legacy publication can still use cached/default policies; explicit memory-only success remains volatile.
  Clones share ordering, not multiple Engines. Fresh-bus tests are not process-crash recovery.
- No live browser-to-Engine, remote CI, optional browser suites, Rust dependency audit, new benchmark,
  deployment/LAN/kernel or release-artifact acceptance was run.

Next: the settings-page load/save boundary, navigation cancellation and partial event/compiler saves.
This pass establishes backend failure reporting, not whole-settings-UI acceptance.
