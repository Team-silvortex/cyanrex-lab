# Chain-guided bug hunt 04: completion and manual header checks

[简体中文](../zh-CN/functional-network-bug-hunt-04.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-04.json)

Date: 2026-09-13. Baseline remains `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Continues on the three uncommitted passes; their reports and original inventory fingerprints remain
historical evidence. No version bump, commit, push, deployment or live Engine mutation was performed.

## Scope

F17 draft/model → F19 semantic suggestions; F16 selected metadata → F18 manual local header checks,
plus marker ownership in the editor controller. Covers E23/E24/E27/E28 and retains E26's explicit-Run
boundary. D07 browser lifecycle changes; no server storage, authentication or Runner implementation changes.

## Reproduced and fixed

| ID | Boundary | Previous behavior | Fix |
|---|---|---|---|
| BH04-01 | Editor/model → language providers/cache | Global providers handled foreign models; pending work and cache survived provider/header replacement; editor disposal left providers registered | Bind all six providers to the owner; independent exact cache and cancellation; invalidate on model/version/context changes and unregister on disposal |
| BH04-02 | Semantic request → suggestions/failure | Shared cancellation, unhandled rejected `finally` promise, stalled requests and malformed items escaped the expected fallback | Independent request generations, ten-second full-request deadline, payload validation; static snippets on service failure, no publication from obsolete requests |
| BH04-03 | Manual check → current draft | Duplicate clicks dispatched twice; late results/errors survived changes/unmount; infrastructure failures appeared as code issues | Key-bound state and request guard, cancellation on code/context change, local 20-second transport; service errors remain errors |
| BH04-04 | Header refresh → current context | Older refresh overwrote newer selection; failed reads silently looked empty; malformed rows crashed rendering | Latest-wins reads, ten-second deadline and validation; retain last successful list with explicit error; refresh revision invalidates checks even with unchanged filenames |
| BH04-05 | Diagnostics → Monaco model | The first global model received markers belonging to a different editor | Always use the controller's owned editor/model |
| BH04-06 | SEC suggestion → inserted C | XDP, TC and tracepoint snippets inserted literal backslash-n text | Real line breaks, preserving snippet placeholders and explicit insertion |

The first 16 browser tests all failed on the old implementation. After the initial fix, two additional
browser tests caught malformed metadata and incomplete provider disposal; one portable test caught
malformed semantic items. These three failed assertions were fixed before final verification. Four more
browser preservation cases cover unchanged-header refresh, timeout/unmount, late errors and normal compiler
rejection. There are 35 new cases: 22 browser controller/provider, 11 portable semantic and two production UI.
These are included in the suite totals below, not additional passes to sum again.

## Behavior and authority retained

- Semantic cache keys use an exact JSON tuple of Engine, header context, model URI, source and cursor;
  only one registration owns its five-second/18-entry cache. There is no global request deduplication.
  Blank or UTF-8 oversized sources do not dispatch semantic checks. Failure preserves static snippets.
- Completion and metadata requests include body reading in a ten-second active-browser limit. Manual
  checks reuse the 20-second local transport. Requests include credentials, request no-store and reject
  redirects. Deadlines do not prove that already dispatched server work rolled back.
- A header refresh invalidates old manual/inline checks and semantic registrations at refresh start,
  even if the returned filenames are identical. The manual button is disabled while metadata loads;
  an error keeps the last successful list visibly labelled and permits a new explicit local self-check.
- Authenticated students can read selected metadata; teachers alone manage header selection/downloads.
  Both check and completion resolve the session owner and selected headers in Engine, ignoring forged
  client ownership/selection. The browser sends only source, plus cursor for completion. It does not
  choose filesystem paths for the driver, switch completion to an Agent or call `/ebpf/run`.

## Verification

| Check | Result | Actual boundary |
|---|---|---|
| Editor intelligence + previous diagnostics browser suites | 38 passed | 22 new + 16 previous; actual React 19 StrictMode/controller/providers, synthetic Monaco/fetch and paused clock |
| All portable frontend tests | 66 passed | Includes 11 new semantic transport/cache tests; no external service |
| Production UI safety + logout suites | 20 passed | Real built Next.js and Monaco on an owned loopback server; all Engine requests mocked |
| Existing compiler route tests | 4 passed | In-process authenticated routes and synthetic driver: owner/headers/cursor, CSRF, capacity, timeout and no fallback |
| Existing header boundary tests | 2 passed | Temporary fixtures: teacher mutation/student read, selection reload and forged-download checksum rejection |
| Production build + explicit TypeScript check | Passed | Current frontend code and four locales |
| Common quality gate | 65 tests and checks passed | Contracts, lengths, versions, course mirrors, tool fixtures and Rust formatting; no live kernel |

The first full UI attempt was 19/20: the new suggestion test invoked Monaco while initial header loading
could still replace its provider, hiding the popup. It now waits for the initial context's completed
inline check before invoking suggestions. The initial failure log is retained separately from the final
passing log. This is test synchronization, not a suppressed production error or an automatic retry.

Portable semantic tests are wired into the existing Node 22 CI job and frontend quality gate; local runs
used Node 24.19.0/Chrome 153. No new dependencies or remote CI execution. Browser tests remain optional;
see [frontend README](../../frontend/README.md). Owned test servers/browsers and generated cache changes
were cleaned up. No real account, instance data or database was read or changed.

## Limits and next segment

Header refresh revisions are browser-local invalidation, not an atomic server-side compile snapshot or
automatic observation of changes from another tab/teacher. Five-second cache reuse does not prove current
server health/header content. Remount isolation is not cross-tab session revocation; stored drafts/targets
remain unchanged. Suspension affects browser timers. Controlled fetch fixtures deliberately permit late
application continuations after abort, whereas production UI uses native browser fetch with mocked HTTP.

No live Engine/Agent/Clang, LAN/SSH, PostgreSQL, kernel loading/events, SDK runtime, dependency audit or
new benchmark was run. Clang completion parsing and Unicode byte-column conversion were not validated in
this pass. The earlier 285-pass/29-ignored full backend result is historical, not added to this run.

Next: F20 manual execution → result display → F22 attachment inventory/detach boundaries, particularly
late results across draft/navigation changes and ownership of refreshes. These are inspection targets,
not confirmed findings or authorization to load programs on a live kernel.

The original map is not re-fingerprinted: 12 cumulative expected source drifts (the previous nine plus
the editor page, controller and intelligence provider). API counts/access, pages, templates and labs
remain unchanged; new helpers/tests and current changed sources are fingerprinted in this pass's record.
