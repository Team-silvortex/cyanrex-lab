# Chain-guided bug hunt 13: settings-page reads, cancellation and partial saves

[简体中文](../zh-CN/functional-network-bug-hunt-13.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-13.json)

Date: 2026-09-24. Base: `main@f56349b4412cb72d2330a7fae11ad597cf6e82a1`, version 0.4.1.
The existing 0.4.1 commit and annotated v0.4.1 tag were pushed before this pass, as requested.
This pass remains local and uncommitted; no further version, commit, tag, push or deployment.
Earlier reports and the frozen 0.3.8 functional inventory remain unchanged.

## Scope and findings

F36 settings UI → verified load → draft → explicit confirmation → event save → optional F03 compiler
save → feedback/reload; E59 history retention and E60 unread invalidation are the adjacent boundaries.
Tests mount the actual SettingsPage, confirmation and i18n providers in React Strict Mode with controlled
HTTP responses and a browser clock. All non-fixture networking is blocked. Two additional existing cases
exercise a local production Next server with synthetic intercepted Engine responses.
No real credentials, deployed Engine/database, Agent, SSH/LAN or kernel mutations.

| ID | Failure-first observation | Fix |
|---|---|---|
| BH13-01 | Failed reads left Save enabled with cached/default values. | Only validated server event settings establish readiness; failed loads lock editing and require explicit reload. Cached drafts are untrusted. |
| BH13-02 | An obsolete Strict Mode request replaced current 500/DropOldest settings with 800. | Abortable read generations bind results to Engine/navigation/reload ownership; stale responses cannot publish. |
| BH13-03 | `ok: "true"` event acknowledgement still dispatched a compiler write. | Validate exact response fields, boolean acknowledgement and the reviewed normalized values before advancing. |
| BH13-04 | Navigation during an event write still allowed its late response to dispatch compiler settings. | Link the confirmation signal to request cancellation and check ownership at each asynchronous handoff. Unmount also prevents late publication. |
| BH13-05 | Compiler failure after event success showed raw backend text and allowed another save without verification. | Report explicit partial/unconfirmed status in four languages, retain confirmed event state, refresh unread, and require reload before a new save. |
| BH13-06 | A stalled initial request had no deadline or Reload Settings action. | Private no-store/non-redirecting GET/POST, ten-second read and twenty-second per-write header/body waits; no mutation retries. |

Compiler-unavailable reads allow clearly labeled event-only saves. Confirmation freezes the normalized
draft and disables editing; blank/fractional limits are rejected, valid integers retain 50..50000 clamping.
Reload is read-only, requires confirmation before discarding edits, and never retries a previous write.
Refreshing metrics cannot clear a partial-save warning. No Engine/API/schema/role or shared confirmation
implementation changed; metrics and Runner Agent transport remain separate.

## Evidence and coverage

The six final baseline browser cases all failed before implementation. Earlier harness diagnostics
include missing cached Chromium, initial stream/React settling, and a later case-sensitive button label
mistake; they are preserved separately and are not product findings. The installed Chrome executable was
used without downloading another browser. The post-fix expanded suite waits for observable page states.

Added **29 unique cases: 12 request/validation tests and 17 actual-page browser tests**. Coverage also
includes success and duplicate-click protection, cancelled reviews, malformed reads, unavailable compiler,
late writes after timeout, partial-save persistence across metrics refresh, unmount/navigation, explicit
reload, dirty-draft discard, locale changes, and mismatched compiler acknowledgements. The twelve request
tests are wired into local/CI frontend checks; optional browser cases require Playwright and Chrome.

| Check | Result | Boundary |
|---|---|---|
| Final failure-first browser baseline | 0 passed, 6 failed | The assertions described above; not every later protective branch independently failed first. |
| Request/decoder/locale tests | 12 passed | Also included in the full frontend gate; do not count twice. |
| Settings-page browser fixture | 17 passed | Expanded suite; three further complete repeat runs are recorded separately. |
| Existing production-page cases | 2 passed | Teacher deployment navigation/settings/terminal and job cancellation/settings review; synthetic API, not live Engine acceptance. |
| Default Rust | 310 passed, 80 ignored | No backend changes; ignored database/kernel/benchmark tests are not passes. |
| Common/frontend/SDK gate | All passed | 69 common, 105 frontend, SDK 13 runtime/3 package plus type checks; 18 production routes; frontend/SDK production audits report zero vulnerabilities. |
| Frozen map checker | Expected drift | 0.3.8 → 0.4.1 and 26 cumulative source changes; no access/API/page/catalog-count drift. |

All 312 earlier report/log/document/mirror/inventory files are verified byte-identical, including whitespace.
Selected source fingerprints and raw logs are in the machine record; this is a dirty-worktree test report,
not release-artifact acceptance. Node 24.19.0, Rust/Cargo 1.95.0 and installed Chrome 153 are used for the
checks; host Node 24.21.0 is also used for report utilities. Final documentation and mirrors are checked
separately without changing tested runtime sources. Only this pass's disposable preview and regenerated
build caches are stopped/removed; source, dependencies, evidence and deployment files remain intact.

## Limits and next slice

- Timeout, cancellation or response loss is **unconfirmed, not rollback**. A late Engine write can still
  complete after a reload. Reads do not establish cross-process coherence or an atomic snapshot of both
  settings; another teacher can change the global compiler mode between steps. No automatic retry.
- Per-request browser timers bound cooperative waiting, not CPU/event-loop stalls, authentication on the
  server or background worker lifetime. Event settings remain per-owner; compiler mode remains global.
- No end-to-end live browser/Engine/PostgreSQL run, new PostgreSQL acceptance, remote CI inspection,
  complete optional-browser suite, Rust dependency audit, deployment, LAN or kernel acceptance.
- Metrics polling and Runner Agent administration are not made safe by this settings-form change.

Next: F03 performance-metrics reads/refresh and panel error recovery, then their boundary with F24/F25
Runner Agent administration. Do not infer full settings-page or all-network acceptance from this pass.
