# Module and boundary test network

Follow-up **2026-09-09**: all nine findings below are fixed and their original regression assertions
pass. See the separate [fix verification](../../reports/acceptance/2026-09-09-boundary-fixes/result.json)
for post-fix runs and added race/cancellation/SQL-cache checks. The running deployment was not updated.
The remainder preserves the **pre-fix** snapshot: failed counts and "unresolved" descriptions below
refer to that earlier run, not current fix status. Its raw evidence has not been rewritten.

Original snapshot: **2026-09-09**, uncommitted **0.3.7** source on `main`.
**Original outcome: failed — 9 regression cases.** This is a source-level test run, not release
acceptance or proof that the running deployment contains these changes.

The network is organized by responsibility, then exercised in dependency order. Unit tests,
in-process HTTP routes, real external boundaries and mocked browser responses are reported separately.
A passing module does not imply every branch, operating system or deployment topology is covered.

## 1. Modules and execution order

Fractions below mean **passed / executed**. Rust portable cases exclude opt-in external tests.

| Order | Module and functions | Rust portable | External boundary | Node / browser |
|---|---|---:|---|---|
| M00 | Configuration, health, environment, metrics, version/doc/contract drift | 6/6 | — | 53/53 tooling checks shared across modules |
| M01 | Password/TOTP, sessions, CSRF, account mutation, teacher authority and solo owner | 36/36 | PostgreSQL 16/20 **failed** | 11/11 unit; 3/3 browser |
| M02 | Minimal teacher discovery, identity/version/protocol checks, invitation and student enrollment | 10/10 | Router/service invitation races; auth cancellation also covered in M01 | 3/3 unit; 8/8 browser |
| M03 | Script ownership, save/list/delete, reload and failed file persistence | 1/4 **failed** | PostgreSQL script persistence and cross-owner isolation 1/1 | — |
| M04 | Module catalog/lifecycle, selected headers, digest validation, terminal commands | 16/16 | Real temporary files; forged downloader, no network download | 2/2 unit |
| M05 | Templates, diagnostics/completion, source validation, debug instrumentation and compiler workspaces | 20/20 | Kernel execution excluded | — |
| M06 | Runner leases/quotas/cancellation, driver dispatch, Agent HMAC/replay, jobs and remote diagnostics | 46/46 | Signed loopback HTTP + real Clang compile 1/1 | — |
| M07 | Labs, assessment, attempts, snapshots, teacher feedback/CAS, owner-bound historical resume | 49/49 | PostgreSQL migration/CAS/reload 1/1 | 9/9 unit; 4/4 browser |
| M08 | Event history/filter/export/delete/unread/settings, fanout, WebSocket and resynchronization | 33/35 **failed** | Real WebSocket paced/burst/stalled scenarios 3/3 | 13/13 unit; 1/1 browser |
| M09 | Layout, permissions, safe confirmation, draft protection, localization and build | — | Production build and TypeScript passed | 6/6 unit; 19/19 browser |
| M10 | OpenAPI, generated types/operations, compatibility, SDK requests and package consumers | 1/1 | 68 OpenAPI operations; 63 generated SDK operations checked | 13/13 SDK runtime + 3/3 package tests; type tests passed |
| M11 | Native release verification/extraction, installation guards, SSH reviewed targets and dependency audit | 28/28 | SSH/Docker/install wrappers use fixtures; audits passed | Included in M00 tooling |

Actual phases: foundation checks → M00–M11 portable groups → PostgreSQL/Agent/WebSocket →
production build/SDK/audits → M01/M02/M07/M08/M09 browser groups → uncovered event HTTP edges →
full new-boundary regression recheck. A failing group did not prevent later groups from running.
The final inventory assigns every Rust case exactly once; exact-filter counts were checked.

Totals: **246/251 portable Rust cases passed**; PostgreSQL **18/22**; Agent **1/1**;
WebSocket **3/3 scenarios of one ignored test**. That is **275 distinct Rust cases executed,
266 passed and 9 failed**, with two performance-only cases not run. Repeated runs are not additional
coverage. Frontend units **44/44**, production-browser cases **35/35**, SDK runtime/package cases
**16/16**, and common tooling cases **53/53** passed.

## 2. Cross-module edges

| Edge | Concrete coverage | Result / qualification |
|---|---|---|
| Browser → auth → teacher/student permissions | Login/session route tests; real SQL mutation/cancellation probes; logout browser error, duplicate-click and navigation cases | **4 SQL failures**; browser Engine is mocked |
| Discovery → compatibility → one-time invitation → account creation | `classroom_tdd`, classroom browser and PostgreSQL cancelled-enrollment probe | Passed tested invariants; cancellation can still leave a committed account, so it is not rollback |
| HTTP session → script owner → JSON/SQL persistence | `module_boundaries/storage.rs`, `script_postgres.rs` | Identity/CSRF/SQL isolation passed; **3 local-file consistency failures** |
| Teacher headers → student metadata → compiler/Runner | Selection reload/removal and checksum rejection; compiler driver receives source, owner, headers and cursor | Passed; downloader is synthetic and checksum-invalid bytes never become a published header |
| Saved source → Runner → attempt → event → feedback → student resume | `module_boundaries/learning_flow.rs` | Passed actual router/services/files; synthetic compile failure, **no kernel execution**; reading history never starts a second run |
| Runner → driver / Agent jobs | Owner/quotas/deadline/cancel tests, unsupported/backend failures without local fallback; signed HTTP probe and Clang output | Passed; Agent isolation descriptor does not prove a container/VM security boundary |
| Event query/settings → retained history → export/delete/unread | `module_boundaries/events.rs` | Read/export/CSRF/owner isolation/retention passed; **2 destructive-filter failures** |
| EventBus → WebSocket → browser recovery | Owner isolation routes; real paced/burst/stalled sockets; browser 1013/snapshot overlap/filter switch | Passed as separate transport and mocked-UI layers, not a real browser-to-Engine end-to-end socket test |
| Learning store → PostgreSQL → feedback revision/reload | Legacy schema migration, concurrent revision writes, missing/foreign historical reads, storage failure | Passed the existing dedicated PostgreSQL case |
| OpenAPI → generated SDK → packed consumer | Contract drift, types, operation dispatch, compatibility, package import/declarations | Passed; 5 signed Agent protocol operations are deliberately outside the 63-operation SDK |
| Teacher CLI → reviewed SSH target → verified package | Argument injection, host-key review binding, exact-once dispatch, version/checksum/private-mode rejection | Passed with fake SSH and packages, **no remote deployment** |

Real WebSocket observations (debug profile, functional pressure checks, **not a release benchmark**):
32 subscribers / 8 owners delivered all 8,000 matching copies in paced mode; all 32 overloaded
connections requested resync in burst mode; the non-reading peer was reclaimed after about 5.06 s,
leaving no subscriber or owner queue.

## 3. Unresolved findings

| ID | Priority | Reproduced behavior | Regression |
|---|---|---|---|
| TN-AUTH-01 | P1 | An old pending login authenticates as a deleted/recreated same-name account | `postgres_old_login_cannot_authenticate_as_a_recreated_account` |
| TN-AUTH-02 | P2 | Pending login after deletion returns 200 with an invalid cookie and disables DB use | `postgres_login_cannot_report_success_after_its_account_was_deleted` |
| TN-AUTH-03 | P2 | Suppressed session cascade can report successful deletion while a valid durable session remains | `postgres_delete_suppressed_session_cascade_cannot_report_success` |
| TN-AUTH-04 | P2 | Suppressed session INSERT can report successful login without a valid session | `postgres_login_zero_row_session_insert_cannot_report_success` |
| TN-SCRIPT-01 | P1 | Saving over an unreadable JSON snapshot returns 200 and overwrites the original bytes | `script_corrupt_snapshot_must_not_be_overwritten_by_a_successful_save` |
| TN-SCRIPT-02 | P2 | Failed file save publishes an unsaved record in memory | `script_failed_file_save_must_not_publish_an_unsaved_record` |
| TN-SCRIPT-03 | P2 | Failed file deletion removes the record from memory | `script_failed_file_delete_must_retain_the_previously_saved_record` |
| TN-EVENT-01 | P1 | In-memory filtered deletion retains matches and deletes their complement; a zero-match filter deletes all records | `filtered_event_deletion_removes_matches_not_the_complement` |
| TN-EVENT-02 | P1 | Invalid severity/category/start/end values become no filter and delete all records | `invalid_event_delete_filters_must_not_widen_into_delete_all` |

The two suppressed-write auth cases use deliberate PostgreSQL triggers, not the default schema.
The auth findings are retained from the preceding investigation; **the 3 script and 2 event cases are
new findings in this run**. The latter five remain ordinary, failing tests, not ignored exceptions.
No business implementation was changed and no fixes are claimed.

The event complement bug is in `EventBus::delete_from_history_filtered`: it calls the read-side
`filter_events` and republishes the matching records as the retained history. Invalid delete values
are silently discarded by the route sanitizers/date parser before reaching the no-filter branch.
Until fixed, avoid filtered event deletion. Fault injection only modified disposable synthetic data.

## 4. Reproduce safely

Use the selected test names/targets in
[`plan.json`](../../reports/acceptance/2026-09-09-test-network/plan.json) and commands/outcomes in
[`result.json`](../../reports/acceptance/2026-09-09-test-network/result.json).
Paths and disposable credentials in evidence are replaced with placeholders; resolve them locally,
do not execute placeholder strings literally.

The new portable boundaries run without a database:

```bash
env -u DATABASE_URL -u CYANREX_TEST_DATABASE_URL \
  cargo test --manifest-path engine/Cargo.toml --locked --test module_boundaries_tdd \
  -- --nocapture --test-threads=1
```

Expected before fixes: **6 passed, 5 failed, 1 ignored**. The ignored case is the separately executed
PostgreSQL script test. For existing portable groups, select the target and exact names from the plan:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib -- --exact TEST_NAME --nocapture
```

For database cases, keep `DATABASE_URL` unset. Set `CYANREX_TEST_DATABASE_URL` only to a newly
provisioned disposable PostgreSQL instance and `CYANREX_DB_FALLBACK=true`; see the
[database isolation procedure](acceptance.md#repeat-the-non-kernel-integrations).
The script database test creates its own schema, verifies SQL rows rather than accepting file fallback,
and drops that schema even when its assertion task fails. Dispose of the database after any outcome.

Browser suites require a **production frontend** built with a fixture-only
`NEXT_PUBLIC_ENGINE_URL`, and matching `CYANREX_UI_ENGINE_URL`, `CYANREX_UI_BASE_URL`,
`CYANREX_PLAYWRIGHT_MODULE` and `CYANREX_CHROMIUM_PATH`. Run each `frontend/tests/*.browser.mjs`
file separately in the plan order. Engine responses are mocked. The initial development-server
run passed 34/35; the unchanged event-stream fixture timed out there and passed under production
(35/35 overall). It replaces global WebSocket, which is also used by development HMR; do not count
that unsuitable-harness result as a confirmed product defect or hide it from the run history.

## 5. Explicitly unproven / not run

- Real LAN teacher/student browser flow with TLS, trust changes, version skew, network partitions and
  a real database; browser mocks and service tests do not establish it.
- PostgreSQL event persistence worker/queue, restart and SQL-to-memory failover consistency; script
  SQL fault/cancellation behavior beyond the successful ownership/reload/concurrent-save case.
- Current-source real eBPF attach/event/detach, multi-kernel and WSL2/Docker Desktop matrices,
  VM/container isolation or escape resistance. Earlier VM evidence remains historical, not rerun here.
- Real SSH upload/install/restart, offline candidate installation and candidate-bound kernel evidence.
  Mock release tools do not establish any of these. No previous VM or running deployment was reused.
- Two ignored event-bus performance-only tests; no new release-profile benchmark baseline.

This run added **12 test cases** without committing, pushing, installing a candidate, restarting the
deployment or loading host-kernel programs. Temporary PostgreSQL used loopback and tmpfs only.
See the result's cleanup and source-input manifest for the final resource and evidence checks.
