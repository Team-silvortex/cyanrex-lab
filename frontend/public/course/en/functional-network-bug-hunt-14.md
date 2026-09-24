# Chain-guided bug hunt 14: performance metrics lifecycle and panel isolation

[简体中文](../zh-CN/functional-network-bug-hunt-14.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-14.json)

Date: 2026-09-24. Base commit: `main@f56349b4412cb72d2330a7fae11ad597cf6e82a1`, version 0.4.1,
with uncommitted pass 13 already present. No version change, commit, tag, push or deployment this pass.
Earlier numbered reports, raw logs, course mirrors and the frozen 0.3.8 inventory are preserved.

## Scope and findings

F03 metrics consumption on observation links E32/E33: private read → validation → hotspot rendering →
manual/automatic refresh → unavailable/stale/recovery. The actual settings page also tests isolation from
F36 settings and the adjacent F24/F25 Runner controls. This co-location is not a new graph edge.
Engine instrumentation, compiler strategy and Runner transport/authority are unchanged.

| ID | Failure-first observation | Fix |
|---|---|---|
| BH14-01 | Malformed operation metrics crashed the settings tree. | Validate both snapshots before state publication and hotspot rendering; preserve the form and Runner panel on failure. |
| BH14-02 | A stalled metrics request never aborted and kept the in-flight refresh slot. | Reuse the private credentialed no-store/non-redirecting settings transport with a ten-second header/body deadline; explicit retry can proceed. |
| BH14-03 | Navigation did not abort the previous metrics read or establish a new owner. | Each Engine/navigation/effect generation owns its request, results and timer; late responses and old refresh callbacks cannot publish or restart it. |
| BH14-04 | Background failure kept old data without an error and retained an earlier refresh-success message. | Visible generic feedback, retained-but-stale samples, neutral non-current labels/colors, recovery only after a validated read. |
| BH14-05 | Switching language issued another metrics read. | Keep localized keys in state and translate at render time; polling no longer depends on the translation callback. |
| BH14-06 | Saving settings cleared an unrelated metrics failure. | Remove cross-panel feedback clearing and place metrics status in its own panel. |
| BH14-07 | Pending event settings disabled independent metrics refresh. | Metrics refresh controls follow their own request state, not settings loading. |

Polling waits ten seconds after completion, including failure, rather than overlapping interval ticks.
Manual refresh coalesces with an active read and resets the next poll's delay. Strict Mode cleanup,
navigation and unmount abort owned work; the last sample is hidden immediately when its scope changes.
No automatic write or mutation retry is introduced; ordinary delayed **read** polling continues.

Counters and displayed aggregate sums must fit nonnegative safe JavaScript integers. Latency accepts
finite nonnegative fractions up to Number.MAX_SAFE_INTEGER milliseconds, above the Engine's u64-nanosecond
representation range. Extra fields are ignored; missing, nonnumeric, unsafe or nonfinite fields are rejected.
No cross-counter ordering/equality is imposed: Engine atomic counters are sampled independently, not as
one transaction. Server metrics semantics and API schemas remain unchanged.

## Evidence and coverage

The final seven baseline browser cases all failed before runtime changes. An earlier equivalent run
checked the new stale flag before the already-existing error field; the final baseline asserts the
missing error first. An initial parser test incorrectly expected MAX_SAFE_INTEGER + 0 to overflow;
the fixture was corrected to add one. Both diagnostics are retained and are not additional product bugs.

Added **25 unique cases: ten parser/request/locale tests and fifteen browser tests**. Browser cases
mount actual hooks, panels or SettingsPage with real confirmation/i18n providers, React Strict Mode,
controlled responses and a browser clock. Non-fixture networking is blocked. No real credentials,
deployed Engine/database, Agent, SSH/LAN or kernel operations are used.

| Check | Result | Boundary |
|---|---|---|
| Final pre-fix browser baseline | 0 passed, 7 failed | First failing assertion in each case; later protective assertions are not separate pre-fix findings. |
| Parser/request/locale tests | 10 passed | Wired into local/CI frontend checks; included in the 115 frontend count below. |
| Expanded browser run | 32 passed | Fifteen new metrics cases plus seventeen unchanged pass-13 settings cases. |
| Three metrics repeat runs | 15 passed each | Repeats, not new unique cases. |
| Existing production-page subset | 2 passed | Teacher settings/terminal navigation and job cancellation/settings review, using intercepted synthetic API responses. |
| Frontend-only quality gate | Passed | 69 common tests, 115 frontend tests, type checks, 18 production routes, zero production dependency audit vulnerabilities. |
| Final documentation preflight | Passed | Course mirrors, versions, length, OpenAPI/SDK drift, common tools/tests and Rust formatting; no Rust build. |
| Frozen map checker | Expected drift | 0.3.8 → 0.4.1, 26 cumulative source changes; no access/API/page/catalog-count drift. |

All 337 earlier inventory/report/log/document/mirror files remain byte-identical. The machine record
identifies selected tested source fingerprints and exact log copies, not a clean release snapshot.
Checks use Node 24.19.0 and installed Chrome 153; host Node 24.21.0 is used for report utilities.
The disposable local preview is stopped; only this pass's generated frontend build/type caches are moved
to the desktop trash (recoverable, not claimed as freed space). No Rust/SDK build cache was regenerated.

## Limits and next slice

- Metrics remain approximate in-process observations, not distributed tracing, proof of current Engine
  health, authorization, or an atomic snapshot. A successful sample can age before the next poll.
- Deadlines bound cooperative browser waiting, not CPU/event-loop stalls or server work. Read failures
  do not undo any independently dispatched settings or Runner action.
- This pass tests the Runner panel's survival and normal reviewed cancellation during a metrics failure,
  not Runner inventory decoding, its own deadlines, failed action acknowledgements or navigation safety.
- Rust/SQL/SDK runtime tests, a full project gate, remote CI, the entire optional-browser suite, real
  browser→Engine integration, deployment/LAN/kernel acceptance and new performance benchmarks were not run.

Next: F24/F25 Runner inventory and probe/cancel lifecycle, especially unconfirmed acknowledgements,
stale inventory, timeouts, and confirmation/navigation cancellation. Read-only code inspection shows
these need their own failure-first tests; this report does not mark them fixed.
