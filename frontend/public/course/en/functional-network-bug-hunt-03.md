# Chain-guided bug hunt 03: browser compiler diagnostics

[简体中文](../zh-CN/functional-network-bug-hunt-03.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-03.json)

Date: 2026-09-13. Baseline remains `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Continues on the two earlier uncommitted passes. Their evidence and the original inventory fingerprints
are preserved. No version bump, commit, push, deployment or real Engine mutation was performed.

## Scope

F17 editor draft → F18 inline local checks / F26 explicitly selected remote diagnostics, primarily
E23/E25 with E26's no-automatic-run boundary. D07 is the surrounding browser lifecycle; this pass changes
in-memory diagnostic state, not stored drafts, authentication cookies or the server's D06 queue.
Manual header checks and semantic completion F19 are separate implementations and not covered by these fixes.

## Reproduced and fixed

| ID | Boundary | Previous behavior | Fix |
|---|---|---|---|
| BH03-01 | Input/Engine/editor → cache | Global cache omitted Engine and editor lifetime; delimiter ambiguity and equal-length 32-bit hash collisions reused another result | Per-editor cache; exact JSON tuple of Engine, target, source and headers; no reuse after remount |
| BH03-02 | Async completion → current draft | Late success/error overwrote the new result; old markers/status survived debounce; stale success populated cache | Key-bound view, immediate stale-marker hiding and post-await cancellation checks before UI/cache publication |
| BH03-03 | Shared promise → cancellation | One editor owned a shared request's AbortSignal; unmounting it aborted the other editor's check | Independent requests and cancellation per editor; remove the global in-flight map |
| BH03-04 | Submission/poll/body → deadline/cleanup | Stalled HTTP submission/poll ignored the loop's 35-second check; late submission entered polling despite cancellation | Whole-request cancellation deadline, pre-poll checks and best-effort cleanup of known late job IDs |
| BH03-05 | Remote terminal state → diagnostics | Cancelled/expired jobs with normalized `result` were shown as code issues | Terminal state takes precedence; unavailable, without diagnostics or a redundant cancellation |

The first 12 real-React browser cases produced 11 failures and one pass on the old hook. These cover the
five findings, including a concrete equal-length collision (`51fedefd-49`) and a surviving consumer whose
shared signal was aborted. All now pass. Four more browser preservation cases, 11 portable transport
tests and one production-editor case were added: 28 new cases total, included in the suite totals below.
Boundary tests use a paused browser clock so host scheduling cannot change exact 699/700 ms or TTL assertions.

## Behavior retained and changed

- Debounce stays 700 ms (1,200 ms for large drafts); blank/over-limit drafts do not dispatch checks.
- A mounted editor still reuses successful or compiler-issue results for eight seconds, at most 24 entries.
  Errors/cancelled continuations are not cached; input identity is exact rather than a short hash. Exact
  keys retain bounded source strings in memory. This trades cross-editor deduplication for independent lifetimes.
- Local inline requests now have a 20-second active-browser limit; remote requests have 35 seconds
  **starting at submission**, including reading the response body and polling. Engine's driver deadline,
  user queue expiry, claimed lease policy, ownership and wire schemas are unchanged.
- Normal compiler rejection is still `issues`, and Clang notes map to informational markers. Remote
  cancellation/expiry is `unavailable`. Checks do not call `/ebpf/run`, relocate semantic completion or
  automatically fall back to local compilation. Explicit remote source-transfer confirmation is retained.
- Requests use credentials/no-store and reject redirects. Known unfinished jobs get one best-effort cancel
  with a separate 10-second deadline; completed terminal jobs need no cancel. No automatic retry is added.

## Verification

| Check | Result | Actual boundary |
|---|---|---|
| Diagnostic browser suite | 16 passed | Real React 19 StrictMode in Chromium; synthetic fetch/clock, no running Engine |
| All portable frontend tests | 55 passed | Includes the 11 new request/transport cases; no browser/network/DB needed |
| Production UI safety + logout suites | 18 passed | Built Next.js/Monaco pages on an owned loopback server; all Engine calls mocked |
| Production build + explicit TypeScript check | Passed | Build/type compatibility, not backend execution |
| Common quality gate | 65 tests and all checks passed | Contracts, mirrors, versions, tool fixtures and Rust formatting; not live kernel tests |

The portable compiler suite is wired into frontend CI and the frontend quality gate. Browser cases
remain optional using an installed Playwright/Chromium; no dependencies were added. Run instructions
are in [frontend README](../../frontend/README.md). Common-check output and exact source fingerprints
are recorded in the machine report. The temporary local UI server and browser processes were stopped.

No Rust implementation changed this pass, so the previous 285-pass/29-ignored backend result is historical,
not a new backend run. No live DB, LAN, SSH, Agent/Clang, kernel load/event, SDK runtime, dependency audit or
new performance benchmark was run. The browser fixture deliberately permits late application responses
after abort; this tests lifecycle rejection, not a claim that native fetch ignores cancellation.

## Remaining boundaries and next segment

Editor remount isolation is not cross-tab session-change detection. An HttpOnly cookie changed elsewhere
is not exposed to the hook; server authorization remains authoritative. Saved drafts and persisted target
selection are not erased. An eight-second cache is not proof of current Agent health or new header content
unless its supplied context changes. Browser suspension affects timers; timeout/cancel does not prove
server rollback. Unknown job IDs still depend on server queue/lease cleanup. Failed remote jobs without
a real compiler report remain indistinguishable from compiler failure in the current normalized wire model.

Next: F19 semantic completion and F16/F18 manual header validation, especially registered editor providers,
late responses after model changes, and whether selected-header context reaches the intended request.
These are follow-up inspection targets, not confirmed new bugs or completed acceptance.

The original map is not re-fingerprinted: nine expected source drifts are cumulative (previous seven plus
the diagnostic hook and quality-gate script). The new transport/helper/tests are recorded in this pass's
candidate fingerprints; API counts/access, pages, templates and labs are unchanged.
