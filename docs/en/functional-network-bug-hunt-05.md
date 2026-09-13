# Chain-guided bug hunt 05: manual run and attachment cleanup

[简体中文](../zh-CN/functional-network-bug-hunt-05.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-05.json)

Date: 2026-09-13. Baseline remains `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Continues on four uncommitted passes. Their reports/logs and original JSON inventory remain historical
evidence. No version bump, commit, push, deployment or live Engine mutation was performed.

## Scope

F17 draft → F20 explicit run → result → F22 owner inventory and explicit cleanup, with F37 confirmation.
Covers browser portions of E26/E39/E62 and existing mock-driver E37 ownership/deadline checks. D07 draft
storage is retained. No Rust implementation, server authorization, API schema or kernel behavior changes.

## Reproduced and fixed

| ID | Boundary | Previous behavior | Fix |
|---|---|---|---|
| BH05-01 | Confirmation → mutation admission/lifecycle | Same-turn calls dispatched duplicate runs/detaches; run and detach could overlap; navigation/unmount did not cancel waiting | Synchronous shared gate, parent cancellation and scoped cleanup; full-request waiting limit |
| BH05-02 | Run → current result | Late results populated another lab; edited drafts retained old results; slow supplementary reads held the running state | Draft/navigation epochs, stale-result notice, discard obsolete results/errors; inventory/progress reads do not hold mutation completion |
| BH05-03 | Response → report/confirmation | Network errors were swallowed before the confirmation failure view; malformed run/debug payloads reached consumers | Typed response validation and explicit uncertainty without retry; normal compiler/source-validation reports and definite authorization errors retain their meaning |
| BH05-04 | Inventory → cleanup controls | Older reads resurrected pins; refresh failures looked successful/empty; malformed rows crashed rendering | Latest-wins bounded reads, row validation and explicit loading/stale/error states; retain the last list and require a successful refresh for cleanup controls |
| BH05-05 | Detach → cleanup claim | Missing `clean` was inferred true; false cleanup with empty notes did not fail; old result pin remained actionable | Require explicit `ok: true` and `clean: true`; failure reaches confirmation; verified removal retires matching result pin/debug session |
| BH05-06 | Source → request size | Character count admitted oversized UTF-8 source | Check encoded bytes before dispatch; reject blank source locally |

The initial 18 browser cases all failed on the old implementation, then passed with the first fix.
Two later preservation cases caught regressions in that first fix: HTTP 400 structured source-validation
reports were lost, and HTTP 401 was wrongly described as an uncertain mutation. Both were corrected;
these are not two extra findings against the original implementation. Four more preservation cases cover
normal compilation rejection, editing during dispatch, late old failures and overlapping inventory reads.
There are 40 new cases: 24 controller-browser, 11 portable transport/validation and five production UI.
These are included in the suite totals below, not additional passes to sum again.

## Behavior and authority retained

- Requests use credentials, no-store and redirect rejection. Mutation waits include body reads in a
  330-second active-browser limit; inventory reads use 20 seconds. These limits do not prove server
  completion, rollback or kernel cleanup. Native browser suspension can delay timers.
- Draft changes do not abort an already dispatched mutation: obsolete output is hidden, then owner
  inventory is reconciled. Navigation/unmount cancels waiting and old continuations cannot update a new
  run or start follow-up reads. There is no automatic retry or detach.
- HTTP 200 compiler failures and valid HTTP 400 failed-run reports remain ordinary reports. Explicit
  400/401/403/404/413/429 errors keep their server message; network, invalid responses, timeouts and 5xx
  failures warn that the effect may already have happened. Failed cleanup never implies rollback.
- The quick detach action requires both a current result pin and a ready inventory containing it.
  Stale/error lists remain visible, labelled, with cleanup disabled; valid empty inventory is distinct.
  Explicit bulk `pin_path: null` retains its disclosed current-account-at-execution semantics, including
  newly added attachments. The displayed list is not a server-side frozen transaction snapshot.
- Engine still chooses the session owner and headers, enforces CSRF/Runner rules, and delegates cleanup
  verification to the selected driver. No client-selected owner or implicit local fallback is introduced.

## Verification

| Check | Result | Actual boundary |
|---|---|---|
| Runtime + previous editor/diagnostic browser suites | 62 passed | 24 new + 38 previous; actual React 19 StrictMode/hooks with controlled responses and paused clock |
| All portable frontend tests | 77 passed | Includes 11 new runtime transport/response tests; hung body and exact timeout covered |
| Production runtime/safety/logout UI | 25 passed | Five new + 20 previous; real built Next.js/Monaco, mocked Engine HTTP/WebSocket |
| Existing Runner lifecycle route tests | 5 passed | In-process authenticated router and synthetic driver: owner, CSRF, unavailable/timeout and unclean report; no host inspection |
| Production build + explicit TypeScript | Passed | Current frontend and four UI locales; no dependency changes |
| Common quality gate | 65 tests and checks passed | Lengths, versions, API/SDK contracts, course mirrors, tool fixtures and Rust formatting |

Local tests used Node 24.19.0 and Chrome 153. Portable runtime tests join the existing Node 22 CI job and
frontend gate; no new CI run or dependency installation. Browser suites are optional, documented in the
[frontend README](../../frontend/README.md). The existing safety suite shares its unchanged mock behavior
through a helper with the new runtime suite. Test browsers/temporary fixtures and the owned loopback
server were closed; no generated TypeScript cache change remains.

## Limits and next segment

The controller guard is not cross-tab idempotency or session-revocation detection. This pass does not
make server attachment inventories atomic, validate live attach success, reconcile kernel state after
process loss or guarantee a combined server pipeline deadline. Rendering a successful mock response is
not proof that a real program is attached. Saved-script/template/learning refresh lifecycles and breakpoint
event filtering remain separate inspection targets; supplementary learning reads are not newly race-free.

No live Engine/Agent/Clang, PostgreSQL, LAN/SSH, kernel load/event sampling, SDK runtime, dependency audit
or new benchmark was run. Earlier full-backend and acceptance results remain historical, not added here.
Next: F21 debug session → F33 live event delivery/recovery → current breakpoint view, using isolated
events and lifecycle tests first. This is a target for inspection, not a confirmed finding.

The original JSON map is not re-fingerprinted. Its checker reports 13 cumulative expected source drifts
(12 from the previous pass, plus the safety-actions hook), with no API/access, page, template, lab or
version drift. Current candidate fingerprints and preserved evidence hashes are in this pass's record.
