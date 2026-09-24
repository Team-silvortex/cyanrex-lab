# Chain-guided bug hunt 07: event history and acknowledgement

[简体中文](../zh-CN/functional-network-bug-hunt-07.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-07.json)

Date: 2026-09-13. Source baseline: `main@705fecc4415d678180c3a6b5f7eefb507f4788e6`, version 0.3.9.
The first six passes are committed in that baseline. The original map remains the 0.3.8 snapshot at
`c26a529`; neither its fingerprints nor earlier reports/logs are rewritten. This pass is uncommitted,
does not bump the version, and does not push, deploy or touch a running Engine.

## Scope

F32 history/filter/export → F34 unread/acknowledgement → F35 scoped deletion/refresh, with F33 recovery
and F37 confirmation as shared boundaries. Covers E54–E58 and E61. Changes are browser-only:
no Rust, route, permission, API wire, SDK, schema, persistence or kernel behavior is modified.

## Reproduced and fixed

| Finding | Boundary | Failure and repair |
|---|---|---|
| BH07-01 | Filters → current history | Old rows committed under new controls, malformed restored dates crashed the page, and relative windows retained expired rows. Scope-bound rendering hides old rows immediately, validates dates/enums/range order, and ages display once per second without querying or acknowledging again. |
| BH07-02 | Export → local download | Duplicate dispatch, obsolete downloads and unbounded waiting. One request owns its exact filter/format context, cancellation and whole-body deadline; MIME/filename checks prevent accidental wrong-format downloads and object URLs are released. |
| BH07-03 | Confirmation → deletion → refresh | Truthy malformed confirmation/counts were accepted; navigation did not cancel waiting. Only exact confirmation and safe counts refresh history; failure remains uncertain without retries. Explicit authorization rejection is separate, and successful refresh invalidates earlier history/badge reads. |
| BH07-04 | History/live → read acknowledgement | Failures were silently ignored, acknowledgements could overlap and pending work survived departure. A filter-bound debounced single-flight request confirms `ok: true`, cancels obsolete waiting and exposes manual retry. Its pre-existing ALL-current-owner scope is explicit. |
| BH07-05 | Unread read → Sidebar | Polls overlapped, accepted malformed counts and silently hid failures. Reads are nonoverlapping and deadline-bounded, unknown state displays `?`, and late responses cannot overwrite a newer post-mutation read. |

The original 13 browser cases all failed against 0.3.9. The first implementation passed 11/13; two
assertions inspected the DOM before asynchronous response/error rendering settled. Those tests now
advance the controlled clock and wait for the actual error view; no product change was needed for
that observation timing. An expanded 28-case run passed. A follow-up check then caught two failures
in the first refactor: read acknowledgement had been bound to navigation but not filters, losing
previous timer cleanup and failing to cancel in-flight work. Binding to the complete filter context
fixed both. All initial/iteration/follow-up/final logs remain separate.

This pass adds 45 cases: 30 real-component browser, 12 portable, and 3 production-page browser cases.
They are included below, not additional to the totals.

## Verification

| Check | Result | Actual boundary |
|---|---|---|
| Event page plus previous controller suites | 112 passed | New 30 + previous 82. Real React 19 StrictMode, Events/Sidebar/confirmation components; synthetic Next routing, page storage, HTTP/WS and paused clock. |
| All portable frontend regressions | 93 passed | New 12 + previous 81. Filter/response parsing, cancellation, complete-body deadline and transport. New entry is wired into CI and the quality gate. |
| Production event/action/breakpoint/runtime/logout pages | 32 passed | New 3 + previous 29. Real Next.js/Monaco and native synthetic JSON download; four locales at 320 px. Every Engine HTTP/WS interaction is mocked. |
| Existing Engine event boundaries | 8 passed | Four authenticated loopback WebSocket routes plus four in-memory module-boundary tests: owner-bound history/export/unread, deletion filters, retention and overflow. No external database. |
| Production build and standalone type check | Passed | 18 generated routes, no dependency/version/type-cache change. |
| Common quality gate | 65 passed + all tool checks passed | Length/version/course parity, OpenAPI/API/SDK compatibility, isolated management/release tool fixtures and Rust formatting; not live kernel or SDK runtime acceptance. |
| Frozen map and historical evidence | Expected drift; 76 files unchanged | The 0.3.8 inventory reports the existing 0.3.9 version change and 19 cumulative source drifts, with no API/access/catalog/page/count drift. Original map and previous reports/logs match the committed baseline byte for byte. |

Runtime: Node 24.19.0, Chrome 153, React 19, Next.js 15.5.24, Monaco 0.55.1. Existing CI uses Node 22;
the browser suites remain optional and this uncommitted pass does not trigger CI.
Reproduction entries and optional browser requirements are in the [frontend guide](../../frontend/README.md).

## Preserved behavior and limitations

- History remains a 200-row best-effort view. Relative aging is a one-second active-browser display tick,
  not server-side retention or atomic snapshot/live joining. Same-scope refresh preserves gap warnings.
- Export remains a separate full-history query; it is not the exact displayed rows. MIME/filename checks
  do not validate all file contents, sanitize spreadsheet formulas or add a total byte budget.
- Export/deletion waiting is 20 seconds; unread/acknowledgement waiting is ten seconds. These bounds
  include response consumption but not an inactive/frozen browser's wall time or server transaction duration.
  Cancellation cannot undo a dispatched deletion or acknowledgement.
- Read acknowledgement still marks ALL current-owner events, including filtered-out/undisplayed records.
  The endpoint has no per-record IDs or cutoff and this pass adds no durable acknowledgement guarantee.
  Failure blocks automatic retries in the same view; manual retry or a new filter/navigation scope may try again.
- Deletion preserves typed review, the exact captured category/severity/time scope, and the existing
  authenticated owner/CSRF boundary. The UI never adds owner selectors, widens a failed query or retries deletion.
  A confirmed response is only the existing Engine acknowledgement, not proof of cross-process SQL durability.
- No real account, event collection, `.env`, database, deployment, Agent, SSH, LAN/TLS or kernel was accessed.
  No benchmark, dependency audit, SDK runtime or full Rust suite was run in this pass; previous acceptance
  remains historical and is not current-source or release-artifact acceptance.

Next: F36 retention/overflow → F32/F34/F35 storage and asynchronous persistence boundaries, using isolated
failure/queue fixtures first. These are pending checks, not confirmed findings or authority to delete real events.
