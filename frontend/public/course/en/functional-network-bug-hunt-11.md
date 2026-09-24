# Chain-guided bug hunt 11: event mutation confirmation and cancellation

[简体中文](../zh-CN/functional-network-bug-hunt-11.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-11.json)

Date: 2026-09-24. Git base: `main@bfecf77292bb87c01d69b8bbd3dc600de1f29e58`, version 0.4.0.
The worktree already contained uncommitted passes 09–10; this is not clean-release acceptance.
No version change, commit, tag, push or deployment. Earlier reports, logs and inventory are preserved.

## Scope and findings

F34 mark-read / F35 deletion → owner admission → persistence barrier → SQL → memory publication → HTTP
acknowledgement. All events/accounts are synthetic. Actual Axum handlers use an explicitly disposable
PostgreSQL 16.15 cluster on a private Unix socket; other services have no deployment database URL.

| ID | Reproduction | Fix |
|---|---|---|
| BH11-01 | Failed mark-read/delete SQL, a failed earlier publication barrier and previously latched fallback returned `200` despite no confirmed durable mutation. | Separate fallible HTTP mutation methods from legacy best-effort helpers. Reject unavailable/invalid configured storage and failed barriers with generic no-store `503`; never substitute volatile success. Definite SQL/schema errors preserve history/unread and remain retryable. |
| BH11-02 | A committed mutation waiting for local cache publication did not finish within a 12-second test bound. | Cap cooperative service waiting, including owner admission, at ten seconds. An admitted SQL worker retains owner admission through publication after cancellation/timeout. Return unconfirmed, not success or a rollback claim. |
| BH11-03 | A successful HTTP mutation had no no-store header. Delete query extraction also used a different plain-text error path. | Successful acknowledgements and handler failures are no-store. Deletion extraction and validation now share generic JSON `400`. The failure-first header assertion covers mark-read; the final expanded HTTP test covers both success paths and invalid/unknown/duplicate query inputs. |

Confirmed SQL deletion counts uncached rows, preserves surviving read flags, drains earlier local
publications and invalidates DropNew counters even for a cold owner. No-match deletion and already-read
acknowledgement remain valid success. Session ownership, CSRF, all-owner mark-read, empty-filter delete-all,
legacy ignored owner fields, successful payloads and SDK signatures remain unchanged. Memory deletion
acquires history/unread/capacity locks before editing; cancellation cannot leave partial cache updates.
This is an added guard, not an independently reproduced pre-fix defect claim.

## Failure-first evidence and coverage

Before implementation, **five PostgreSQL cases and one HTTP case failed**. The contract/CI check had
two failures among five cases. After the first fix, all five SQL cases passed. Four additional SQL cases
cover admitted cancellation, SQL timeout, schema repair and cold-cache counts; three ordinary service
guards cover invalid configuration/filter validation, atomic memory cancellation and admission timeout.
The first expanded SQL run passed 41 cases; the final run with the cold-cache guard passed all 42.

Added **14 unique cases: four ordinary Rust, nine ignored PostgreSQL and one Node contract test**.
Existing auth/deletion contract and CI-selection assertions were updated, not counted as new cases.
The initial mistyped map-check command is retained separately as an operational diagnostic, not a defect.

| Check | Result | Boundary |
|---|---|---|
| Focused HTTP | 9 passed | One new and eight existing event/learning cases; overlaps the complete gate. |
| Final default event/filter suite | 44 passed, 44 ignored | Includes previous fanout/filter/retention coverage; ignored cases are 42 SQL and two manual benchmarks. |
| Real event PostgreSQL | 42 passed | Previous 33 plus nine new. Cancellation and SQL/post-commit waiting exercise both mutations. |
| Contract and CI selection | 7 passed | New named SQL cases selected with nonempty guards; not a remote CI run. |
| All default Rust | 307 passed, 70 ignored | Explicit SQL execution is separate from default passes. |
| Common, frontend and SDK gate | All passed | 68 common, 93 frontend, SDK 13 runtime/3 package plus type checks; 18 production routes; frontend/SDK production audits reported zero vulnerabilities. |
| Frozen map | Expected drift | 0.3.8 → 0.4.0 and 24 cumulative source changes; no access/API/page/catalog-count drift. |

The full gate uses Node 24.19.0 and Rust/Cargo 1.95.0; source/report utilities also use host Node 24.21.0.
Final documentation mirrors and common checks were rechecked after recording results, with runtime
sources unchanged. All 263 earlier inventory/report/log/history-mirror files remain byte-identical,
including uncommitted pass 09–10 evidence. The machine record preserves source and evidence fingerprints.
The private cluster had zero test schemas/other clients before stop/removal. Only this pass's temporary
cluster and regenerated build caches were removed; source, dependencies, evidence and deployment remain.

## Limits and next slice

- Unconfirmed is not rolled back: a SQL statement or local publication may complete after cancellation,
  timeout or response loss. Tests deliberately verify post-deadline completion; SQL timeout tests do not
  claim server-side rollback. Inspect state before an explicit retry; no automatic retry was added.
- Ten seconds bounds cooperative service waiting, not authentication, serialization, CPU stalls or a
  whole-worker lifetime. An admitted worker still waits for cache locks. Waiting cancellation before
  admission dispatches nothing; admitted SQL cancellation is not termination of the operation.
- Definite query/schema failures remain retryable, but existing barrier/storage timeouts or independent
  writer failures can latch fallback. Repair/restart does not reconcile volatile events. There is no
  new shutdown hook, network-partition/lost-commit injection, crash/power-loss recovery or durable replay.
- Clones of one EventBus share ordering; external writers and multiple Engines do not. Legacy infallible
  mutation APIs and settings/publication fallback retain their previous contracts. Explicit memory-only
  HTTP success remains volatile, not durable storage.
- Frontend source was not changed; existing tests verify error handling, not a new live browser-to-Engine
  run. Remote CI, Rust dependency auditing, optional browsers, new benchmarks, SSH/LAN/kernel/deployment
  operations and release-artifact acceptance were not run.

Next: F36 event retention/settings and overflow-policy acknowledgement under storage fallback, again
using isolated fixtures. This pass does not establish those settings endpoints' durable-success contract.
