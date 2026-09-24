# Chain-guided bug hunt 15: Runner inventory, probes and cancellation safety

[简体中文](../zh-CN/functional-network-bug-hunt-15.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-15.json)

Date: 2026-09-24. Base: `main@f56349b4412cb72d2330a7fae11ad597cf6e82a1`, version 0.4.1,
with uncommitted passes 13 and 14 already present. No version change, commit, tag, push or deployment.
Earlier numbered reports, raw logs, course mirrors and the frozen 0.3.8 inventory remain unchanged.

## Scope and findings

F24/F25 teacher-browser inventory → review → probe/cancel → acknowledgement → read-back. E50 is covered
only at the browser's identity/freshness consumption boundary, not Agent registration, signatures, claim,
execution or result authentication. SettingsPage also verifies isolation from F03 metrics and F36 settings;
co-located panels do not create a new graph edge. No backend authority or queue contract is changed.

| ID | Failure-first observation | Fix |
|---|---|---|
| BH15-01 | Malformed nested Agent data crashed the settings tree. | Decode both complete inventories before publishing; validate nested fields, counts, identities, dates and bounded collections. Ignore undisplayed job output and extension fields. |
| BH15-02 | Stalled inventories never released refresh or aborted. | Private credentialed no-store/non-redirecting requests with ten-second header/body deadlines; failure cancels the other half of the pair. |
| BH15-03 | Navigation left the previous inventory request alive. | Engine/navigation/effect ownership excludes old results and timers; Strict Mode starts a fresh owner. |
| BH15-04 | Failed refresh left old probe/cancel buttons actionable. | Retain a clearly labeled reference inventory but block actions until verified reads recover. |
| BH15-05 | A 2xx response without a job acknowledgement never reached the failed-confirmation state. | Require the actual Engine job DTO and matching target/request or job identity; never infer success from status alone. |
| BH15-06 | Clicking a health probe dispatched immediately without review. | Add a cancellable explicit Agent confirmation and recheck current identity, healthy state and expiry before dispatch. |
| BH15-07 | Unmount left a cancellation POST alive. | Connect confirmation and generation cancellation to bounded POSTs; late results cannot publish or start read-back. |

## Safety semantics

- Agent/job responses publish as one UI pair, **not** a transaction across two server GETs. A malformed
  or failed half cannot publish a partial update. Existing valid empty/disabled inventories remain valid.
- Polling waits ten seconds after completion and coalesces reads. Dispatch is blocked during reads;
  polling pauses during writes. A confirmation whose reviewed job identity/state/assignment changed,
  or whose Agent re-registered, must be opened again. Client rechecks do not replace server authorization.
- Probes check Agent expiry against the inventory's Engine timestamp plus monotonic browser elapsed
  time from request start; synchronized browser/server wall clocks are not assumed. Dispatch also rejects
  a sample aged twenty seconds or more. Server state can still change after this check.
- Writes have twenty-second header/body deadlines. Probe acknowledgement binds the queued control-probe
  job, Agent, message and timeout. Cancellation accepts the Engine's `cancelled` or `cancel_requested`
  response for the reviewed immutable job identity; it does not claim that an active Agent has stopped.
- A confirmed write stays confirmed if the follow-up inventory fails; stale actions remain blocked.
  An unconfirmed write may already have taken effect and requires an **explicit successful refresh**
  before retry. Background read success cannot remove that warning or unlock actions. No automatic POST
  retry, compensating cancellation, local fallback or server rollback is introduced.
- Navigation, Engine changes and unmount abort browser waiting, not admitted Engine work. Locale changes
  translate panel feedback without refetching or discarding a neighboring settings draft.

## Evidence and coverage

The final seven baseline browser tests all failed before runtime changes. The first baseline had four
incorrect button labels; the next still had an insufficient acknowledgement assertion that passed while
the old implementation waited for read-back. The final baseline waits for explicit failed-confirmation
feedback. These intermediate diagnostics are preserved, not counted as additional product findings.

Expanded checks caught two decoder gaps (normalized impossible calendar dates and excess nested labels),
then passed after tightening validation. A navigation test initially asserted before React removed the
dialog; waiting for detachment fixed that test without changing the runtime. Legacy browser fixtures now
return actual Engine inventory/job DTOs rather than incomplete objects or `{ok:true}` cancellation replies.

Added **34 unique tests: 10 unit and 24 browser cases**. Browser cases mount actual panels/SettingsPage,
confirmation and i18n under Strict Mode, with a controlled clock and synthetic responses. External
non-fixture networking is blocked. No real credentials, deployed Engine/database, Agent, SSH/LAN or kernel
operations are used.

| Check | Result | Boundary |
|---|---|---|
| Final pre-fix browser baseline | 0 passed, 7 failed | First failing assertion per case, not independent reproduction of all later protective assertions. |
| Expanded regression run | 66 passed | 10 Runner unit + 24 Runner browser + 17 settings + 15 metrics cases; no failures. |
| Three Runner repeat runs | 24 passed each | Repeat evidence, not additional unique tests. |
| Existing production-page subset | 2 passed | Teacher navigation and full-job-ID cancellation/settings confirmation, local production Next build with synthetic intercepted APIs. |
| Frontend-only quality gate | Passed | 69 common + 125 frontend tests, type checks, 18 production routes, zero production dependency audit vulnerabilities. Unit tests are wired into local/CI checks; browser suite remains optional. |
| Final documentation preflight | Passed | Mirrors, versions, lengths, OpenAPI/SDK drift, common tools/tests and Rust formatting; no Rust build. |
| Frozen map checker | Expected drift | 0.3.8 → 0.4.1 and 26 cumulative source changes; no access/API/page/catalog-count drift. The finite map does not fingerprint every new file. |

All **361** earlier report/log/numbered-document/mirror/inventory files remain byte-identical; see the
frozen receipt. Raw logs are exact copies with SHA-256 receipts. Selected source fingerprints identify
tested files, not a clean release snapshot. Main checks use Node 24.19.0 and installed Chrome 153;
report utilities and the intermediate decoder-failure run use host Node 24.21.0.

The owned loopback preview is stopped. This pass's generated frontend build/type caches are moved to
desktop trash (recoverable; this does not claim reclaimed disk space). Dependencies, runtime data and
prior evidence remain. No Rust or SDK build cache was regenerated.

## Limits and next slice

This is browser lifecycle verification, not full Runner acceptance. No Rust/SQL/SDK runtime suite,
full project gate, remote CI, live browser→Engine integration, signed Agent execution, deployment/LAN,
kernel tests or performance benchmarks were rerun. Deadlines cannot bound a blocked browser event loop.
Fresh reads are observations, not proof that an uncertain operation never occurred or cannot finish later.

Next: F14/F15/F16 teacher module/header management and structured terminal commands, focusing on stale
inventories, confirmations, failed acknowledgements and navigation boundaries. This pass does not mark
those paths complete.
