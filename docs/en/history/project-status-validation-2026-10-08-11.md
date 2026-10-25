# Project status validation records 2026 10 08 to 11

Archived on **2026-10-25** from `docs/en/project-status.md`, original lines **239–1046** at source commit `be94db9`.
Original dated release and implementation validation records for October 8–11.

These are historical statements, not current capability or fresh acceptance claims. Original versions, dates,
Unreleased labels, hashes, failure counts and verification limits are retained. Only relative Markdown
link targets are rebased for this directory. Use [current project status](../project-status.md),
the [reference index](../reference-index.md) and [development history](../development-history.md) for navigation.

<!-- original-project-status:start -->
## Verification recorded so far

The C2-T–W changes and Next.js update are included in 0.5.4. Their dated implementation records below
retain their original 0.5.3 workspace version, Unreleased status, counts and failure history; those
records are not a new 0.5.4 release gate or evidence of publication or deployment.

The catalogue/editor/shared-admission follow-ups are now included in 0.5.5. Their dated 0.5.4
workspace records retain their original Unreleased labels, counts and SQL scope.

### 0.5.6 version validation · 2026-10-11

`bash scripts/quality-gate.sh --no-npm-install` passed for source **0.5.6**.
Default Rust: **832 passed, 0 failed; 742 ignored**, **75 result blocks**, no compiler warnings.
Common scripts: **248 passed**. Frontend: production build, full typecheck and **208 tests** passed.
SDK: generated/API/surface checks, build, **39 tests**, type tests and **3 consumer package tests** passed.
Fresh frontend and SDK production npm audits both reported **zero vulnerabilities**.
Version, course, OpenAPI, formatting and tooling checks passed; public API baseline remains **0.3.0**.
Frontend Node module-type warnings remain; this gate does not claim warning-free JavaScript execution.
Index/file preparation is included in 0.5.6; earlier 0.5.5/Unreleased execution records retain their scope.
Not rerun: opt-in SQL/file cases, real-kernel/distribution artifact acceptance, browser/LAN or deployment.
No preparation API mounting, network transfer, durable grant issuance, recovery or live-auth cutover.
Evidence: `/tmp/cyanrex-release-056-vh5118WO/quality-gate.log` (local log, not a release artifact).

### Unreleased upload-bound local receipts · 2026-10-11

Source **0.5.5**, outside the tag; [original upload/file pins and one-shot attempts](../large-file-upload-staging.md).
Compiled Red: **0 passes / 1 expected failure**, seven temporary fixture warnings; Green **3 default / 9 SQL/file cases**.
Adjacent intent regression: **12 exact SQL cases**; full gate **798 default Rust / 229 common**, **700 ignored**, **70 result blocks**, no warnings.
Real lock-wait drift, same-byte inode replacement and running-worker cancellation are covered, not durable progress, Session, resume or transport.
All **102 prior dated blocks** and tensor evidence remain unchanged; detailed scope and the initial CI length-guard failure are in the linked record.
Private PostgreSQL stopped; **47 MiB** cluster and **3 MiB** regenerable cache removed, logs retained under `/tmp/cyanrex-upload-bound-wQ9u2Poq`.
No version, commit, push, release or deployment change; no disk-fault, power-loss, benchmark, browser, TLS/LAN or desktop acceptance.

### Unreleased durable upload intent and logical quotas · 2026-10-11

Source **0.5.5**, outside the tag. The separate [upload ledger](../large-file-upload-intent.md) adds
trusted immutable owner/account-generation reservations, persisted logical limits, serialized
admission and bounded full-ledger post-write checks. It does not connect chunks or current Session.
Compiled Red: **0 passes / 1 expected behavior failure**, with six temporary unused-fixture warnings.
Green: **5 default cases / 12 exact real PostgreSQL cases** pass; the 64 GiB fixture is metadata only.
Full backend gate passes **795 default Rust / 225 common checks**, **691 opt-in cases ignored**,
**69 Rust result blocks**, no compiler warnings; tool fixtures are not live acceptance.
No lost-commit-ack, filesystem quota/fault/power-loss, benchmark, browser/SDK, Session, TLS/LAN,
deployment or desktop acceptance; earlier index SQL was not rerun. Details and early guard failures
are recorded in the upload document. All prior dated blocks and capability-tensor evidence remain unchanged.
The private PostgreSQL 16 instance had no TCP listener; every upload schema was dropped. Its stopped
47 MiB cluster was removed; regenerable cache/empty fixtures were cleaned, with logs retained under
`/tmp/cyanrex-upload-intent-7iIyc9OE`. No release/version, commit, push or deployment change.

### Unreleased private Unix chunk staging · 2026-10-11

Source **0.5.5**, outside the published tag. The separate [staging API](../large-file-staging.md)
binds exclusive entries to the entire typed manifest/index, pins an existing private root, verifies
exact bytes, synchronizes file/directory and reads back the originally created inode. Clone-shared
worker permits belong to actual queued/running jobs; independent handles have additive budgets.
Existing entries conflict; uncertain output remains without overwrite, adoption, retry or cleanup.

The compiled baseline first ran **0 passes / 2 expected behavior failures**. Green passed **22
default integration cases** (two baseline plus twenty extensions) and **one private queued-worker
case**. Fixtures include a real 4 MiB + 17-byte file, complete-manifest aliases, retained corrupt/
short/long output, private-root/entry drift, FIFO with no writer, actual same-byte/different-inode
substitution retaining the original single-link inode, and running cancellation/timeout barriers.
Manual future polling with one blocking thread proves queued cancellation without a timing guess.

The backend-only gate passed **790 default Rust cases and 221 common script checks**, with **679
opt-in cases ignored**, 68 Rust result blocks and no compiler warnings. Formatting, file lengths,
version/API/SDK drift, generated course copies and synthetic tooling passed. All **98 preexisting
bilingual verification blocks** and the capability tensor were preserved without changing evidence.

Inherited deployment/database settings were removed. No real SQL, frontend/browser, SDK runtime,
advisory scan, disk-full/fsync fault, power-loss recovery, RSS/throughput, TLS/LAN, desktop or deployment
acceptance was rerun. Synthetic fixtures/cache were removed and logs retained. No upload journal,
reserved quota, durable progress/resume, publication/grants, transport/client or live composition
was added; current byte verification is not a historical receipt. No commit, push or version bump.

### Unreleased large-file manifest and streaming core · 2026-10-11

Source version **0.5.5**, outside the published tag. Following
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), the independent
[core](../large-file-core.md) implements typed file/version metadata, strict bounded decoding,
fixed-buffer inspection, exact chunk copying and ordered chunk/whole-file verification. Unix
preopened-file observations check source metadata, not paths, private roots or immutable storage.

Two behavior tests first compiled and ran with **0 passes / 2 expected failures** against the
replacement stubs. The completed target passed **37 default cases**, plus **two private Unix
snapshot-boundary cases** in the library. Fixtures include a real **4 MiB + 17-byte** file, independent
byte/hash oracles, short reads/writes, partial unverified output, EOF faults, hostile IO counts,
strict/duplicate metadata and a sparse over-limit descriptor rejected before reading. The real
same-length mutation test exercises the snapshot helper, not a concurrent in-read race.

The completed backend-only quality gate passed **767 default Rust cases and 220 common script
checks**, with **679 opt-in cases ignored**, 67 Rust result blocks and no compile warnings.
Formatting, file lengths, version/API/SDK drift, course copies and synthetic tooling passed.
Two earlier four-case inventory runs each had 3 passes/1 failure: an unfinished test-file inventory
and a wrong CI-command assumption in the new guard. Its corrected checks passed before the gate.
All **96 preexisting bilingual verification blocks** and the capability tensor remained unchanged.

Inherited deployment/database settings were removed. No PostgreSQL, frontend/browser, SDK runtime,
advisory scan, actual 64 GiB read, RSS/throughput benchmark, TLS/LAN, desktop or deployment acceptance
was rerun. Fixtures were removed; logs were retained. Copy success is not flush/fsync, publication
or delivery. Durable staging, worker/quota admission, resume, publisher/recipient authorization,
transport/client wiring and desktop pairing remain absent. No commit, push or version bump occurred.

### Unreleased private task metadata HTTP · 2026-10-11

Source version **0.5.5**, outside the published tag. The separate
[HTTP contract](../task-content-index-http.md) projects one existing current-Session index command at
exact `GET /platform/v1/task-index`. Strict raw `after`/`limit` parsing grants no owner or namespace
selection. Dedicated credentials, empty-body checks and shared response-lifetime admission remain
independent of the old manual/catalogue collection contracts and live application.

The closed-source dispatch test first compiled and failed with **400 versus expected 503**,
then passed after the real command was connected. A temporary unused fixture import was corrected
without suppressing warnings. Review also strengthened browser-entry scanning and exact response
envelope/null assertions; all eight new SQL cases passed again after that assertion change.

Fresh backend verification passed **728 default Rust tests** and **216 common tests**, with
**66 successful Rust result blocks and zero compilation warnings**; **679 ignored Rust cases
are not passes**. The isolated PostgreSQL runners passed **51 distinct cases**: new index HTTP 8,
existing trusted/Session index 12, manual content HTTP 13, catalogue content HTTP 8 and text Artifact
HTTP 10. Real blockers and database expiry verify cancellation/reauthorization; snapshots protect
source/resource rows and file bytes/modes. The response is metadata, not content availability or a receipt.

The disposable database had no remaining fixture namespaces or clients before shutdown. Its temporary
cluster and synthetic files were removed; logs remain. Bilingual contracts and course copies are
synchronized. All **94 prior status sections** and the tensor snapshot/topology/scores/evidence remain
unchanged. No HTTP mounting, issuer, live OpenAPI/SDK/UI list, installation, migration or recovery was
added. Frontend/SDK runtime, cross-target builds, large-list benchmarks, remote CI, kernel, SSH/LAN
and deployment acceptance were not rerun. No version bump, commit or push is part of this slice.

### Unreleased owner-private task metadata index · 2026-10-11

Source version **0.5.5**, outside the published tag. The independent
[index contract](../task-content-index.md) adds metadata-only trusted and current-Session pages:
default 20, limits 1–32, UUID keyset order, exact-owner filtering and validated lookahead.
Explicit neutral catalogue configuration admits manual and complete configured bound/unbound
definitions; the original content read/write modes and storage schemas remain unchanged.

Two closed-source tests first compiled and failed on placeholder results, then passed after real
dispatch. A later exact SQL test exposed missing content columns returning an empty success for
an absent owner. It failed with `Ok(empty)` versus `StorageUnavailable`, then passed after fixed
zero-row Task/outbox column checks. No other owner's rows are decoded for those probes.

Fresh verification passed **714 default Rust tests**, **212 common tests** and the backend gate;
**671 ignored Rust cases are not passes**, with **65 successful Rust result blocks and no warnings**.
The explicit isolated PostgreSQL runners passed **86 distinct cases**: new trusted index 4, new
Session index 8, existing content store 21, Session content 33 and catalogue tasks 20. After the
empty-page fix and bounded test-loop hardening, all **12 new index cases** passed again through
the exact runner. Real blockers and database time verify expiry/cancellation, not timing guesses.

The disposable database had no remaining fixture namespaces/clients before shutdown; the failed
test's synthetic schema and temporary cluster were removed, with logs retained. Current bilingual
contracts and course copies are synchronized. All **92 prior status sections** and the tensor's
snapshot, topology, scores and evidence remain unchanged. This slice adds no public HTTP/SDK/UI,
content-byte proof, installation, migration or recovery. Frontend/SDK runtime, cross-target builds,
large-inventory benchmarks, remote CI, kernel, SSH/LAN and deployment acceptance were not rerun.

### 0.5.5 version validation · 2026-10-11

Version metadata, current documentation and generated course copies are synchronized at **0.5.5**.
The fresh full local gate (`--no-npm-install`, unchanged verified dependency graphs) passed
**706 default Rust, 208 common, 208 frontend and 42 SDK tests**; **659 ignored Rust cases** are
not passes. Rust produced **63 successful result blocks** without compilation warnings.
Frontend/SDK production dependency audits each reported zero vulnerabilities; the existing Node
`MODULE_TYPELESS_PACKAGE_JSON` warnings remain.

The production-build browser fixtures additionally passed **23 task-payload and 18 multi-language
editor cases** with real pages/Monaco and mocked Engine reads. These verify local editing, not server
saving. This release validation does not rerun opt-in PostgreSQL, kernel, SSH/LAN, remote CI, Rust
advisory or deployed restart/recovery acceptance. Prior dated SQL records and the tensor snapshot,
scores and evidence remain unchanged. Source publication does not mount prepared APIs, migrate a
database or update a running deployment.

### Unreleased catalogue publication target observation · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Independent C2-AA adds
`observe_unconfirmed(&self, token)` to the still-owned catalogue publication attempt. Ready and
Complete return `NoUnconfirmedStep` without I/O; an unconfirmed attempt rejects any token other
than its original canonical Session before dispatch. A matching fingerprint is continuity, not
authorization. One existing current-Session command reads the fixed exact Artifact revision or
the current catalogue Task and its ordered contents, retaining existing final authority checks.

The body-free report uses the existing generic step/outcome values, not a manual attempt or a
checkpoint/journal conversion. Matching requires full frozen `Some(definition)`, original manifest,
Draft revision 1 and all ordered original bytes. Another registered complete definition or a legal
later edit can differ. Same-reference metadata drift, manual-mode records and damaged bytes remain
typed reader errors; they are not converted into normal differences or invisibility.

An exact historical Artifact revision may match after its head changes. The observer does not scan
earlier confirmations. A missing Task read verifies source and Task scope without entering Artifact
namespace validation, while an existing empty Task does validate that namespace. Read locks and
commit boundaries remain those of the original commands, not SQL READ ONLY or a whole-workflow
transaction. Results may become stale; NotVisible proves neither rollback nor permanent absence.
Success, error and cancellation preserve the complete definition, target, stopped state and original
confirmation prefix. Even a later matching resource is not adopted as a receipt and cannot resume
`advance`.

TDD first compiled and executed one meaningful Red: the stub returned `NoUnconfirmedStep` instead
of the real closed-pool Artifact reader's typed storage error. The implemented reader then passed
that same exact case without warnings. This was an executed behavioral failure and subsequent Green,
not a fixture, missing-method or compilation failure.

The expanded public target initially passed four cases with one unused shared-fixture reexport
warning. A narrowly scoped allowance on that reused test-fixture module removed the warning without
changing the old fixture. Final public execution passed **four default cases** with **eight SQL cases
ignored**, and private comparison/admission execution passed **six cases**, without warnings. The six
new structural guards also passed. These scoped runs are not additive to a complete backend gate.

The independent disposable PostgreSQL runner passed **eight distinct exact cases**: state/original-
Session preflight; matching, valid different and exact historical Artifacts without prefix scans;
bound/unbound multi-package/version, empty and ordered Task contents and legitimate edits; missing/
foreign targets and missing versus existing-empty Task scope; effective deferred commit faults,
damaged/missing files and full-definition/manual-mode corruption; current logout/expiry/membership
and database-clock expiry during exact reader waits; observed cancellation; and separate pending
creators that commit or abort. Both whole-binding directions first match legitimately, then mutate
Task and outbox copies and return typed `InvalidRecord`. Fault counters and blocking barriers prove
the intended injection occurred; an invisible read is not used as rollback proof.

Adjacent exact runners separately passed **twelve old manual-observation**, **eight catalogue-
publication** and **thirty-three Session-content** cases: **61 distinct SQL cases** total including
the new eight. Repeat runs and name-list checks are not additional cases. Read-preservation checks
compare every table's sorted-row MD5 in the selected source/Artifact/Task schemas and private file
names, lengths, modes, SHA-256 and root device/inode/mode. These are scoped fingerprints, not raw-row
equality, an audit of every deployment schema or proof of historical publication provenance.

Capability validation passed **18 Node cases** and **412 existing distinct paths**. Only W16 gains
five source/documentation and five test references; scores, older evidence, snapshot fields and
topology remain identical to this round's baseline. All **88 earlier dated status sections** and
the published changelog tail remain unchanged. The ten old manual production files, shared pure
checks and catalogue confirmation file also retain their baseline hashes.

The complete backend gate passed **706 default Rust tests** and **208 common tests**, with **63
successful Rust result blocks** and no compiler warnings. **659 ignored Rust cases** are not counted
as default passes; the 61 exact SQL cases above ran separately. Rust formatting, generated contracts,
version synchronization, course copies and tooling checks passed. This round did not run a frontend/
SDK build, browser suite, real kernel/LAN acceptance, deployment, security audit or release gate.

Execution used an environment stripped of inherited deployment/database settings, a synthetic
private PostgreSQL 16 database with a Unix-only socket, and this round's owned private temporary
files. Before stopping the database, no fixture schema or other client remained. Its process was
confirmed stopped and its owned data/socket directories removed; project storage was not touched.
The final format/common gate again passed 208 checks without warnings; this repeat is not additional
test evidence. After it completed, the owned empty fixture/runtime directories and environment helper
were removed too. Cleanup removed **137,814,141 apparent bytes (about 131.4 MiB)** in total; this
round's logs remain in `/tmp/cyanrex-catalog-observation-xOASCU8O`. Only disposable fixtures were removed.

The [contract](../session-catalog-task-draft-observation.md) is an internal read-only preparation step.
No HTTP listener, browser save, SDK, live authentication, installer or migration is connected.
The source version is unchanged; no commit, push or release is part of this development round.

### Unreleased independent catalogue draft publication · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Independent C2-Z preparation returns
an opaque `SessionCatalogTaskDraftPublication` from the trusted catalogue-content workspace, an
exact reference and a C2-O plan. Structural token/source checks and complete exact definition lookup
precede target allocation; preparation performs no SQL, file or network I/O. Unknown coordinates
return `InvalidPreparation`, without provider discovery, manual fallback or inferred latest versions.

The frozen full definition may bind both assessment contracts or neither, but the final Task must
retain `Some` of that exact metadata. Each advance dispatches at most one existing current-Session
Artifact create, or the final catalogue-content Task create. It marks the exact target Unconfirmed
before awaiting; exact acknowledgement records progress without another await. Errors, mismatches
and dropped dispatched futures retain confirmed prefixes and stop. Wrong-token and unpolled calls
do not dispatch. Current authorization and actual owned text are independently rechecked by the
existing commands; no assessment, Review or acceptance is performed.

Only Artifact receipt matching and manifest construction are shared pure checks. Manifest binding
now rejects short or excess receipt prefixes before zip. The manual final matcher remains None-only;
the catalogue matcher requires the complete frozen Some definition. No old attempt conversion,
checkpoint/journal/observation API, durable receipt, recovery, retry, cleanup, whole-draft transaction,
HTTP/SDK/browser wiring, installer or migration was added. The eight protected old manual
checkpoint/dispatch/intent/inspection/observation source files remain byte-identical to this round's baseline.

The meaningful Red compiled and failed **0/1** on the temporary preparation stub's
`InvalidPreparation`, without compile warnings. The first attempt instead failed while opening its
fixture: a newly created parent directory was group-writable and was correctly rejected. That setup
failure is not feature Red. A premature Green build encountered not-yet-written test modules; a
later unit build found three missing test imports. Neither executed cases or established Green.
Both were corrected. Initial target Green passed four cases with one unused import warning; removing
that import and rerunning yielded **four default cases** and **eight ignored SQL cases**, without
warnings. The separate private receipt tests passed **eight cases**, including bound/unbound positive
controls, full-definition/owner/manifest negative controls, dropped futures and prefix cardinality.
The adjacent old manual unit filter passed **33 cases**; these default checks are not additive to the
complete backend gate recorded below.

The new disposable PostgreSQL runner passed **eight distinct exact cases**: mixed packages/versions/
bindings and ordered Unicode/same/empty text; unknown coordinates and Session substitution; second
Artifact failure; final definition/outbox/Session/membership/deferred-commit faults; damaged bytes and
preallocated Artifact/Task conflicts; current-Session revocation; observed cancellation waits; and
database-clock expiry while blocked. Counters prove injected faults were reached. Comparisons cover
full rows in seven Task/Artifact resource tables and private-directory names/bytes, not every identity
table. A failed Artifact step preserves the exact observed orphan as well as prior confirmed data;
a failed final Task step does not erase its Artifact prefix. Neither error nor cancellation implies
whole-draft rollback or permission to retry. Independent source/test review found no remaining
ineffective injection or new production defect.

All four adjacent PostgreSQL runners passed **67 distinct exact cases**: new catalogue publication
eight, old manual publication 12, Session content 33 and journaled dispatch 14. Each checked its exact
case name before execution; all completed without Rust compile warnings. These synthetic in-process
transactions do not establish a mounted service or live deployment.

The complete backend gate passed **696 default Rust and 202 common tests**, across **62 successful
Rust result blocks**, without compile warnings. **651 ignored Rust cases** are not counted as passed.
File/version/course synchronization, public API/SDK generation and compatibility, Rust formatting
and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional suites,
browser or dependency audits, live kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks do not validate a real deployment.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Only W15 gains five source/document and five test/runner references, bringing distinct
paths to **404**. All other fields, scores, evidence, snapshot date and topology remain unchanged.
Tensor/reading-view checks passed **18 cases**; these are also included in the common gate, not
additional unique tests. Each locale's 43 prior dated sections remains byte-identical.

The disposable PostgreSQL 16 fixture used a private Unix socket only. Before stopping it, the
observed test-schema and other-client counts were both zero. Approximately **166 MiB** of this round's
owned database/test data, socket and launcher were removed; they are recreatable fixtures, not user
runtime data. Logs remain. The final format/common preflight also passed; its repeated 202 common
cases are not counted twice. Bilingual documents and built-in course copies are synchronized. This
round did not start the application service or change the source version, commit or push anything.

### Unreleased explicit catalogue Task-content HTTP · 2026-10-11

Source version remains **0.5.4**, outside the published tag. The independent C2-Y adapter takes
`CatalogTaskContentHttpState` with a frozen `SessionCatalogTaskContentWorkspace` and exposes four
create/read/replace/status commands only on its separately constructed `/platform/v1/catalog-tasks`
router. Creation requires an exact `definition_ref`, never a client definition, provider or mode
selector. Reference and package records use strict object decoding over the original JSON stream;
missing, unknown, duplicate, positional or invalid fields fail before Session dispatch.

Bound and explicitly unbound entries from composed package/version catalogues retain complete
definition metadata and private ownership. Existing Session transactions verify full metadata,
old/new exact text, revision fences and fresh authority. No assessment, Review or acceptance runs.
The old manual state, three-field creation JSON and prefix remain unchanged; wrong-mode records
fail closed. Unknown configured references map to generic `400 invalid_request / unconfirmed`
after the Session command was called, even when its internal lookup precedes SQL work.

The adapter reuses the dedicated Cookie/Origin/marker rules, 64 KiB/two-second body boundary,
private response projection and response-body-held admission. Clones share four default slots,
configurable from one to 16; separately constructed manual/catalogue/text budgets add. Saturation
does not poll the body or queue a Session command. Cancellation releases a slot, not a rollback
receipt. There is no normal-app mounting, issuer, discovery, automatic router merge, C2-P–W
publication/journal integration, live OpenAPI/SDK/browser connection or migration.

The focused Red test compiled and failed **0/1** because the temporary stub returned 404 instead of the
required missing-reference 400. Its minimal temporary harness emitted five unused-code/import
warnings. Green passed **28 default Rust cases**, with **eight ignored SQL cases** not counted as
passed and no compile warnings. The defaults comprise one missing-reference case, three exact
reference/mode cases, eight corrected catalogue-body cases and six transport plus ten admission
cases reused against the actual catalogue router. Duplicate-field injections assert their wire
changes, and valid nested-manifest controls prove rejection is not merely a missing reference.

The new disposable PostgreSQL runner passed **all eight distinct exact cases**. They cover mixed
packages/versions/bindings through all four commands, manual/catalogue and owner separation,
full metadata/binding drift and unknown coordinates, invalid/corrupt text, current Session and
account lifecycle, post-write definition/old/new input/session/membership and commit faults,
observed lock-wait expiry, post-write loss of an input removed by replacement, and response-body/
SQL-wait/cancellation admission. Fault counters prove mutations reached injected failures;
rejections compare full rows in **seven Task/Artifact resource tables** and all private-directory
file names/bytes, not every authentication/identity table. These observed fixture states, not
HTTP error outcomes, establish the tested nonchanges. Independent source/test review found no
new production defect or remaining ineffective injection.

All four relevant PostgreSQL runners passed **64 distinct exact cases**: new catalogue HTTP eight,
manual HTTP 13, Session content 33 and text HTTP ten. Each runner checked the exact case name before
execution; all completed without Rust compile warnings. These are in-process routers and synthetic
transactions, not a live listener, deployment or browser workflow.

The complete backend gate passed **684 default Rust and 196 common tests**; **643 ignored Rust
tests** are not counted as passed, across **61 successful Rust result blocks**, with no compile
warnings. File/version/course synchronization, public API/SDK generation and compatibility, Rust
formatting and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional
suites, browser or dependency audits, live kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks do not accept a real deployment.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Nine new W13 source/test/document paths bring distinct paths to **394**; only W12's gap
wording additionally reflects the separate typed adapters. Scores, evidence, snapshot date and
topology remain unchanged. Tensor/reading-view checks passed **18 cases**. Each locale's 42 prior
dated sections remains byte-identical; the new adapter does not inherit their execution evidence.

The disposable PostgreSQL 16 fixture used only a private Unix socket; fixture schemas and other
client connections were zero before shutdown. Approximately **153 MiB** of this round's owned
database/test data, socket and launcher has been removed; these are regenerable fixtures, not user
runtime data. Logs are retained. No application service was started. The final format/common
preflight also passed; its repeated 196 common cases are not counted again. English/Chinese
documents and their built-in course copies are synchronized. No version, commit or push was made.

### Unreleased optional automatic-assessment bindings · 2026-10-11

Source version remains **0.5.4**, outside the published tag. `TaskDefinition` now has paired
`Option<VersionedName>` evidence-schema and assessment-policy fields. Definitions may supply both
exact pins or explicitly bind neither; half bindings fail both decoding and trusted registration.
Both JSON fields remain required: two explicit nulls mean no automatic assessment, while omission
is rejected. Old bound JSON retains its exact field order and values; existing records need no
migration. The Rust source fields require callers to adapt. New unbound records require the new
reader; old binaries fail closed, so backward reading/downgrade of those records is not promised.
Core and storage schema numbers remain unchanged.

Mixed metadata still feeds the existing bounded catalogue composition, Task drafts, schema-2
Session catalogue and schema-3 content-store/Session paths. An unbound definition remains
`Some(TaskDefinition)`, not a manual Task. Full saved-definition equality makes adding or removing
a binding under the same reference a metadata mismatch, not a policy upgrade. Typed provider
registration rejects unbound entries, and assessment checks both pins before calling the provider.
Actual `TaskAssessment` pins stay mandatory. There is no placeholder pass, rule Review, acceptance,
provider discovery, new authorization branch, public HTTP or browser-save connection.

Two focused Red tests compiled and failed separately (**0/1** each): trusted registration still
required an assessment binding, and ordinary optional-field decoding silently accepted omission.
This round adds **ten default Rust cases and two exact SQL cases**. Selected default regressions
passed **54 cases**: metadata 26, original typed catalogue nine, teaching compatibility six, Session
catalogue three, Session content four and content storage six. Pure cases cover required JSON keys,
duplicate/unknown fields, the exact old bound encoding, explicit null pairs, six-slot sequence
compatibility, half/truncated/extra values, namespace and byte/count limits, duplicate full references,
typed rejection before assessment and mandatory result pins.

The four adjacent PostgreSQL runners passed **86 distinct exact cases**: schema-2 Session catalogue
20, schema-3 Session content 33, content storage 21 and manual draft publication 12. The two new
cases are included in that total. They round-trip unbound catalogue definitions through drafts,
input/text replacement and Ready transitions, retain exact definition and private ownership, and
reject both adding and removing a binding under an unchanged reference. Each rejected operation
preserves all row values in seven business tables and every existing file name/byte; unaffected
definitions remain readable. Manual/catalogue separation and existing Session checks remain intact.
These new cases do not separately re-accept all lifecycle/authorization boundaries; adjacent suites
provide that coverage. Independent source review found no new production or boundary defect.

The complete backend gate passed **656 default Rust and 192 common tests**; **635 ignored Rust
tests** are not counted as passed, across **60 successful Rust result blocks**, with no Rust compile
warnings. File/version/course synchronization, public API/SDK generation and compatibility, Rust
formatting and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional
suites, browser or dependency audits, kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks are not evidence of a real deployment.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Four new test references bring distinct paths to **385**; scores, evidence, connections,
snapshot date and every non-implementation field are unchanged. Tensor/reading-view checks passed
**18 cases**. Each locale's 41 prior dated status sections remains byte-identical, including earlier
Unreleased rounds. English/Chinese documents and their built-in course copies are synchronized.

The disposable PostgreSQL 16 fixture used only a private Unix socket. Fixture schemas and other
client connections were both zero before shutdown. This round's approximately **138 MiB** of owned
database/test data, socket and launchers has been removed; these fixtures are regenerable and need
no migration, while execution logs are retained. No application service was started and user
runtime data was not changed. The final format/common preflight also passed, without counting its
repeated 192 common cases again.

### Unreleased bounded multi-package catalogue composition · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Trusted
`TaskDefinitionCatalog::compose(&[catalogues])` combines frozen metadata for multiple domains,
package versions and task-definition versions. It accepts 1–256 input handles and checks the actual
flattened total of 1–256 definitions before allocation/copying. Nested composition has the same
global limit. Disjoint registrations of the same package/version are allowed; repeating a full
definition reference is rejected even for identical metadata, with no deduplication, overwrite or
latest-version fallback. Input and definition order remain intact in the owned immutable snapshot.

The existing Task draft, schema-2 Session catalogue and schema-3 content-store/Session constructors
accept this composed handle without new routing or authorization branches. Each saved definition
still must match in full. Old single-package registration and provider-bound typed assessment remain
strict and separate. Composition retains no provider, interprets no payload and adds no discovery,
publisher trust, public HTTP, browser saving, schema migration or publication/retry policy.

The first multi-package behavior test compiled and failed against the placeholder (**0/1**), then
passed after implementation. This round adds **eight default Rust cases and two exact SQL cases**.
Selected default regressions passed **44 tests**: 16 metadata, nine original typed catalogue, six
teaching compatibility, three Session catalogue, four Session content and six content-store cases.
Pure cases include all exact reference coordinates, source/entry order, frozen clone/drop behavior,
256/257 boundaries, nested limits, duplicate equal/drifted definitions and unchanged typed dispatch.

Both new database cases passed individually. In one workspace each creates four Tasks from definitions
across two packages, two package versions and two task versions; exact reads, input/text reordering and
state changes preserve full definitions and private ownership. Missing packages, missing exact
package versions and real metadata drift reject read/replace/transition, while another admitted
package remains readable. Seven business tables' complete row values and all existing file names/
bytes remain unchanged on rejection. During source review, a policy-version fault that originally
assigned its current value was changed to a genuinely different value; explicit inequality assertions
now prove both metadata injections alter the fixture. This required no production change.

The two new cases do not independently reaccept every authorization/lifecycle boundary; adjacent
existing suites supply that coverage. Independent production and test reviews found no new issue
or relaxation of manual, typed-provider or public boundaries. Execution counts and broader acceptance
remain scoped to this dated round, rather than inherited from earlier runs.

The four complete adjacent PostgreSQL runners passed **84 distinct exact cases**: 19 schema-2
Session catalogue, 32 schema-3 Session content, 21 content-store and 12 manual draft-publication
cases. The two new cases are included in those totals; individual reruns add no cases. The complete
backend gate passed **646 default Rust and 189 common tests**, with **633 ignored Rust cases**
excluded, **60 successful Rust result blocks** and no Rust compilation warnings. File/version/course,
public API/SDK generation and compatibility, Rust formatting and existing tool-fixture checks passed.
This round does not rerun frontend/SDK functional suites, browser or dependency audits, kernel,
SSH/LAN, remote CI or restart/recovery acceptance. Tool fixtures are not live deployment evidence.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**.
Four new test references bring the distinct referenced paths to **381**; scores, evidence, edges,
snapshot dates and all non-implementation fields remain unchanged. Tensor/reading-view checks passed
**18 tests**. The 40 preceding dated status sections in each language remain unchanged, including the
prior Unreleased rounds. English/Chinese documentation and the built-in course copies are synchronized.

The disposable PostgreSQL 16 fixture used only a private Unix socket. Fixture schemas and other
client connections were zero before shutdown. Approximately **118 MiB** of this round's owned
database/test data, socket and launchers were removed; regenerable fixtures need no migration, and
execution logs remain. No application service was started and user runtime data was not changed.

### Unreleased catalogue and text-content composition · 2026-10-08

Source version remains **0.5.4**, outside the published tag. Explicit
`TaskContentStore::from_definitions` and `SessionCatalogTaskContentWorkspace::from_definitions`
combine frozen exact definitions with the existing schema-3 content manifest. The four new Session
commands create, read, replace and transition catalogue content without constructing a provider.
Definition admission and text-content metadata are independent choices: title/filename/language
labels do not replace definition display metadata, execute assessment or create a Run.

The original manual handles remain definition-free. Unknown creation coordinates are rejected before
database work; stored missing/unknown/drifted definitions and wrong modes fail closed. Full saved
definition equality, sorted old/new Artifact-byte checks, Task revision fences, atomic Task/manifest/
outbox records and final current-Session authorization plus confirmed commit remain shared. The
catalogue workspace cannot enter the manual HTTP or C2-P–W publication/observation entry points.
No schema change, migration, public mount, issuer, browser saving or recovery/retry policy is added.

The first Store and Session behavior tests each compiled and failed against their placeholders
(**0/1** each), then passed after implementation. Selected default regressions passed **10 tests**.
This round adds **eight default Rust cases and six exact SQL cases**; ignored tests are not counted
as default passes. **86 distinct exact PostgreSQL cases passed**: all 31 Session-content cases,
21 adjacent content-store cases, 18 schema-2 Session catalogue cases, 12 manual draft-publication
cases, two selected journaled-dispatch cases and two selected journal-observation cases. The last
two selections are not full reruns of their respective suites; repeat selections add no cases.

New database coverage includes exact-version/empty/32-item round trips, manual/catalogue/schema-2
isolation, six full-definition drifts, unknown coordinates, corrupt old or invalid new content,
owner/account lifecycle privacy, post-write record/event/file faults and Session expiry after an
observed storage wait. Rejected writes preserve complete business rows and existing files; fault
sequences prove the selected mutations reach their injection points. An independent source review
found no new production issue or public/manual boundary relaxation.

The first backend preflight passed **185/186 common tests**: one old journal-read source assertion
still matched the old helper signature. It now explicitly requires the manual-only `None` argument;
the production path needed no adjustment. The complete rerun passed **638 default Rust and 186
common tests**, with **631 ignored Rust cases** excluded, **60 successful Rust result blocks** and
no Rust compilation warnings. File/version/course, public API/SDK generation and compatibility,
Rust formatting and existing tool-fixture checks passed. This round does not rerun frontend/SDK
functional suites, browser or dependency audits, kernel, SSH/LAN, remote CI or restart/recovery
acceptance; earlier dated runs are not fresh evidence for these changes.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**.
Source, test and evidence references total **377 distinct paths**, and the W11/W12 gap descriptions now reflect
explicit catalogue mode; scores, evidence, edges and earlier dated records remain unchanged.
English/Chinese documentation and bundled course copies are synchronized. The disposable PostgreSQL
16 fixture used only a private Unix socket; after verifying zero fixture schemas and other
connections it was stopped. Approximately 119 MiB of owned database/test data, socket and launchers
were removed, with logs retained. No application server was started in this round.

### Unreleased task definition and payload decoupling · 2026-10-08

Source version remains **0.5.4**, outside the published tag. Frozen `TaskDefinitionCatalog` metadata
can now admit Task drafts and Session catalogue commands without constructing or retaining a
`TaskProvider`. The existing typed `TaskCatalog<P>` still owns evidence and assessment. Exact
single-package/version lookup, mandatory evidence-schema/policy pins, full saved-definition equality,
private ownership, input-byte checks and transaction-time Session checks remain unchanged.

Portable payload hints now follow the existing server ASCII contract, independently of 14 installed
editor profiles. Unknown valid hints retain their raw values and use the existing plain-text profile
with an explicit notice and disabled language tools. Synchronization compares resolved profile IDs,
preserving the model and undo history; only an explicit language edit changes the saved hint. This
does not add a payload kind, format version, LSP transport or server connection.

The first metadata behavior test compiled and failed against the placeholder (**0/1**), then passed.
The frontend draft selection first passed **21/23**, with two new compatibility assertions failing,
then passed after implementation. Final selected Rust regressions passed **26 default tests**:
eight new metadata, nine original catalogue, three Session catalogue and six teaching compatibility.
The first Session selection failed all three cases because this round's temporary parent directory
was mode 0775; correcting that owned fixture to 0700 made the complete selection pass without
weakening production file checks.

**35 distinct exact PostgreSQL cases passed**: 18 Session catalogue (including two new provider-free
round-trip/metadata-drift cases) and 17 adjacent input-replacement cases. Coverage retains binary and
empty inputs, exact versions, old/new constructor interoperability, owner privacy and unchanged
business rows/files on rejected full-definition drift. Repeated selections are not extra cases.

The backend gate passed **630 default Rust and 183 common tests**, with **625 ignored Rust cases**
excluded, **60 successful Rust result blocks** and no Rust compilation warnings. The frontend gate
passed **208 functional tests**, production build/type checks and a zero-vulnerability production
dependency audit; existing Node module-type warnings remain. **41 real-browser cases** passed across
task payload, multilingual editor, filename-import and navigation suites, including unknown-hint
round-trip/edit/undo, rejected edits, model disposal and four-locale 320px layout. The browser used
mocked identity responses, not a live Engine or backend-save acceptance; the payload suite issued no
Engine writes. Public API/SDK generation and compatibility, file/version/course and tool checks passed.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**;
only implementation source references expand to **367 paths**, without changing scores or evidence.
Earlier dated records remain unchanged. This round does not rerun SDK functional tests, Rust dependency
audit, kernel, SSH/LAN, remote CI or restart/recovery acceptance. No mount, issuer, storage migration,
automatic rule execution, save/retry policy or authorization relaxation is added.

The disposable PostgreSQL 16 fixture used only a private Unix socket. After verifying zero fixture
schemas and other connections, it and the owned loopback Next.js server were stopped; approximately
103 MiB of database/fixture data, socket and launcher were removed, while reproducible logs remain.

### Unreleased C2-X text Artifact HTTP · 2026-10-08

Source version remains **0.5.4**; this follow-up is outside the published tag. The independently
constructed text router creates one private fixed-profile Artifact and reads one exact historical
revision through existing current-Session commands. Its **2 MiB wire limit**, **256 KiB decoded-text
limit** and separate state-family admission budget do not expand C2-N's 64 KiB manifest limit.
Returned references can feed a separate Task request; this is not an atomic batch or router merge.

The first real behavior test failed against the placeholder: a valid 70 KiB text request returned
`service_unavailable / not_attempted` instead of reaching the closed source with `unconfirmed`.
After implementation, **12 default transport/contract tests, 10 exact PostgreSQL cases and three
new common guards passed**. Test review then strengthened two existing assertions: valid JSON
precedes trailers/stream errors, and rejected-input preflight holds an exact source-schema lock.
The full twelve-default/ten-SQL selection passed again, with unchanged case counts and no production
change required for those assertion improvements.

Coverage includes empty/Unicode/exact-limit text, strict JSON and client-authority rejection,
historical revision pins, foreign/missing privacy, current Session/membership and expiry behind
observed storage waits, duplicate conflicts, outbox/deferred-commit and file/schema faults, retained
response capacity and cancellation. Trigger counters prove selected write faults execute; retained
orphan files are neither adopted nor deleted. The bridge is an independent in-process fixture HTTP
sequence: text publication, then a separate C2-N Task create/read using exact returned references.

**56 adjacent exact SQL cases also passed**: 13 C2-N HTTP, 18 Session Artifact and 25 C2-M content
cases. Including C2-X, this round ran **66 distinct SQL cases**; targeted/full reruns do not increase
that number. The final backend-only gate passed **622 default Rust and 180 common tests**, with
**623 ignored Rust cases** excluded and **59 successful Rust result blocks**, without Rust compilation
warnings. File/version/course, public API/SDK generation and compatibility checks and existing tool
fixtures passed. The capability map now records **70 coordinates, 69 functions, 39 evidence entries,
114 edges and 19 representative chains**; W23 alone adds scoped **3/1/3/1** evidence, while all older
coordinates, scores, evidence and dated records remain unchanged.

PostgreSQL 16 used a disposable private Unix socket with no TCP listener. After confirming zero
fixture schemas and other connections, the server was stopped and approximately 120 MiB of owned
database/fixture data, socket and launcher removed; reproducible test logs remain.
This round does not rerun
frontend/SDK functional suites, dependency audits, actual browser/proxy/TLS, kernel, SSH/LAN, remote
CI or restart/recovery acceptance. Generic Artifact reads may materialize 1 MiB before the stricter
text projection. No live mount, issuer, browser saving, installation/migration, journal enforcement,
automatic retry/adoption/cleanup or global worker/memory quota is added. Cancellation is not rollback
proof and does not stop dispatched blocking I/O; earlier verification records remain historical.

### Unreleased HTTP shared admission 2026-10-08

Source version remains **0.5.4**; this follow-up is not in the published tag. The standalone C2-N
adapter now shares a fail-fast budget across cloned states and their routers: four slots by default,
trusted configuration from one to sixteen. Slots cover body collection, C2-M, serialization and
returned response bodies through their terminal frame/end, error or drop. Excess requests return
private `503 / capacity_exhausted / not_attempted` before body polling or SQL.

The first real Rust behavior test failed against the original implementation: with four body reads
held, a fifth request still reached the closed source and returned `service_unavailable / unconfirmed`.
It passed after implementation, followed by **24 HTTP default tests** (14 preceding, 10 new),
**one new outgoing-body error unit** and **three new common wiring guards**. The unit separately
keeps a failed body alive while checking that its slot is already released.

**13 exact HTTP PostgreSQL cases and 25 adjacent C2-M cases passed**, totaling **38 distinct SQL
cases**. Two new HTTP cases retain committed response bodies, reach an observed event-write blocker,
reject overflow without business changes and verify completion or cancellation with source-pool
restoration. The cancellation fixture observes unchanged Task/outbox/files; cancellation alone is
not rollback proof. A test-only abort-on-drop guard also prevents an early assertion failure from
detaching the pending request. The HTTP runner and CI inventory now enumerate all thirteen cases.

The backend-only local gate passed **610 default Rust and 177 common tests**, with **613 ignored
Rust cases** excluded and **58 successful Rust result blocks**, without compilation warnings.
File/version/course checks, public API/SDK generation and compatibility checks, and existing tool
fixtures passed. The current capability tensor remains 69 coordinates, 38 evidence entries, 111
edges and 18 representative chains, with unchanged scores. The historical 0.3.8 network checker
reports source/version drift as expected for that preserved snapshot; its hashes were not refreshed.

This round does not rerun frontend/SDK functional suites or dependency audits, actual browsers,
kernel, SSH/LAN, remote CI or restart/recovery acceptance. No listener, issuer, public save connection,
schema/journal change, global process/worker/memory quota or retry policy is added. Cancellation does
not terminate dispatched blocking I/O. The earlier dated evidence and published version record stay unchanged.

### 0.5.4 version validation · 2026-10-08

Version metadata, current documentation and generated course copies are synchronized at **0.5.4**.
The fresh full local gate passed **599 default Rust, 174 common, 206 frontend and 42 SDK tests**;
**611 ignored Rust cases** are not passes. Rust produced **58 successful result blocks** without
compilation warnings. Frontend/SDK production dependency audits each reported zero vulnerabilities;
the existing Node `MODULE_TYPELESS_PACKAGE_JSON` warnings remain.

This version-only validation does not rerun opt-in PostgreSQL, actual browser, kernel, SSH/LAN,
remote CI, Rust advisory or deployed restart/recovery acceptance. The separately dated C2-T–W
implementation evidence below keeps its original version, counts and scope. Version publication
does not mount prepared APIs, migrate a database or update a running deployment.

### Unreleased C2-W journal step observation · 2026-10-08

Source version remains **0.5.3**. **Two default Rust tests, twelve exact PostgreSQL cases and three new
common guards passed** for the separate journal-bound resource observer. The twelve ignored SQL cases
in the default target are not default passes; the final Rust runs emitted no compilation warnings.
A real behavior test failed against the stub before the implementation passed.

The first SQL runner reached six passes before the seventh case failed during fixture setup: it tried
to revise an Artifact's kind, which the existing command correctly rejects as `InvalidInput`.
The fixture now retains `cyanrex.task-text` while changing title/bytes to create a later head.
That exact case passed, followed by a fresh complete **12/12** runner. This fixes test preparation,
not production behavior, and neither relaxes the kind rule nor adds a distinct case.

Coverage distinguishes missing/filtered intents, unrecorded ordinals and invisible resources; it
checks exact historical Artifact revisions, current Task edits and all referenced contents, typed
blob faults, missing versus existing-empty Tasks, final authorization, waiting and cancellation.
Fresh Sessions are seeded fixtures, not a public issuer or real OTP-login test. Account-generation
rejection changes a stored binding field, not an actual account deletion/recreation cycle.

Concurrency checks show external journal metadata/nonce/DDL writers blocked during resource waiting;
after reader cancellation the writer can proceed and a later read rejects the damaged record.
This is not in-transaction DML-trigger or OID-replacement fault injection. First-snapshot/full-prefix
reuse is constrained by source guards alongside normal read execution. The observer still performs
no business write and returns no body, nonce, receipt, restored attempt or retry permission.

All **61 adjacent exact PostgreSQL cases passed**: 25 C2-M content, 12 C2-S checkpoint inspection,
10 C2-V journal inspection and 14 C2-U dispatch cases. Together with C2-W, this round covers
**73 distinct SQL cases**; targeted reruns do not add distinct cases. The isolated PostgreSQL 16
used a private Unix socket without TCP. Cleanup confirmed zero custom fixture schemas and zero other
connections before stopping the server and removing approximately **185 MiB** of owned database data.

The full quality gate exited successfully: **599 default Rust tests passed**, with **611 ignored**
tests excluded from that count, across **58 successful result blocks**. **174 common, 206 frontend
and 42 SDK tests passed**. Next.js **15.5.27** production build, frontend type checking, SDK build/type/package
checks, generated contracts, API compatibility against **0.3.0**, course/version checks and tools passed.
Fresh frontend and SDK production dependency audits each reported zero findings at every severity.
Rust emitted no compilation warnings; the existing non-failing Node `MODULE_TYPELESS_PACKAGE_JSON`
warnings remain.

This round did not rerun actual browser, kernel, SSH/LAN, remote CI, Rust advisory audit or deployed
restart/recovery acceptance. Source version remains **0.5.3**, with this work unreleased and not committed or pushed.

### Unreleased C2-V journal inspection · 2026-10-08

Source version remains **0.5.3**. **Two default Rust tests, ten exact PostgreSQL cases and three new
common guards passed** for the independent current-Session journal reader. The ten ignored cases in
the default target are not counted as default passes. Final Rust runs emitted no compilation warnings.
A real behavior test first failed against the `UnsupportedSchema` stub, then passed with implementation.

The first ten-case SQL run passed nine cases; its concurrency fixture failed while trying to corrupt
an owned synthetic file installed with mode `0400`. Only that fault-injection fixture changed its file
to `0600`; production permissions were unchanged. The exact case then passed, followed by a fresh
complete **10/10** runner. The reruns are not extra distinct cases or a production permission fix.

Coverage includes an empty plan's zero records versus its one final Task, a closed journal pool with
the source-owned transaction, primary-key/nullability/scope metadata checks, invalid prefixes and
plan-specific row bounds. Duplicate rows behind invalid uniqueness are rejected as `UnsupportedSchema`,
not evidence of reaching a duplicate-row decoder branch. Synthetic rows test the 32-item/33-step limit
and rejection of a 34th row; they are not 33 real resource writes. Account-generation tests alter the
stored binding, not an actual account deletion/recreation cycle.

Present and absent intent reads recheck expiry after waiting. Readers wait for B to commit or roll back;
cancellation preserves business state, and damaged current resources do not trigger a resource/file
probe. Results remain journal metadata, without nonces, restored authority or permission to retry.
All **26 adjacent exact PostgreSQL cases passed**: fourteen C2-U dispatch and twelve C2-T intent
cases, for **36 distinct SQL cases** in this round. Tests used an independent PostgreSQL 16 server
on a private Unix socket with no TCP listener or deployment URL. No custom fixture schemas or other
connections remained; the server was stopped and its roughly 133 MiB temporary data and remaining
synthetic files removed. Diagnostic logs were retained; project data was not deleted.

The fresh full local gate passed **597 default Rust, 171 common, 206 frontend and 42 SDK tests**.
Rust produced 57 successful result blocks without compilation warnings; **599 ignored cases are not
counted as default passes**. Next.js 15.5.27 production build/TypeScript, SDK build/type/package,
generated contracts, compatibility against baseline 0.3.0, version metadata, course mirrors and tool
fixtures passed. Fresh frontend and SDK production audits each reported zero findings. Existing Node
module-type warnings remain. These results do not establish actual browser/kernel acceptance,
remote CI, Rust advisory audit, host crash, backup replay or deployed recovery; those were not rerun.
Source remains based on `fa15fe35c492eb1b61114c8430d67eb7936f11c4` plus uncommitted C2-T/U/V work.

### Unreleased C2-U journaled dispatch · 2026-10-08

The final fresh full local gate passed **595 default Rust, 168 common, 206 frontend and 42 SDK tests**.
Rust produced 56 successful result blocks with no compiler warnings; **589 ignored cases are not
counted as default-suite passes**. Next.js 15.5.27 production build/TypeScript, SDK build/type/package,
generated contracts, compatibility against baseline 0.3.0, version metadata, course mirrors and tool
fixtures passed. Fresh frontend and SDK production audits each reported zero findings.

Source version remains **0.5.3**, based on `fa15fe35c492eb1b61114c8430d67eb7936f11c4` plus the
uncommitted C2-T/U work. **Three new default Rust tests, fourteen exact PostgreSQL cases and three
new common guards passed**. Real fresh-install and dispatch behavior Reds preceded the implementation.
The consumed original attempt first confirms a unique Unknown reservation; a separately authorized
transaction then verifies its exact binding and commits one resource's SQL with its marker. Existing
C2-M policy is shared rather than bypassed. The original C2-P path remains independently callable.

The new cases cover fresh schema 2 and unchanged schema 1 rejection, empty and Unicode drafts,
exact owner/account/nonce/prefix checks, actual Session removal between A and B, wait expiry,
trigger faults, deferred commit failure and cancellation in each phase. Post-resource intent/marker
changes and equivalent step-table replacement roll back the business SQL; a 34-row overflow is
rejected without truncation. A final targeted rerun also proved rejection when a resource trigger
changes only a previous confirmed nonce. Account-generation tests mutate stored binding fields,
not genuine account deletion/recreation. Definite B rollback retains A's Unknown and may retain files;
an uncertain B COMMIT can already have committed its marker and is not rollback or retry evidence.

All **84 adjacent exact PostgreSQL cases passed**: 25 C2-M, 11 standalone content HTTP, and 12 each
from C2-P/Q/S/T, for **98 distinct SQL cases** including C2-U. These ran on a private PostgreSQL 16
Unix socket with no TCP listener or deployment URL. No custom fixture schemas or other database
connections remained; the server was stopped and its roughly 168 MiB temporary data removed.
Diagnostic logs were retained; project data was not deleted. Reopened handles and controlled faults
do not test host crash, backup replay, physical source continuity across A/B, or restart recovery.

The first full gate stopped at a fresh Next.js 15.5.24 production audit: one moderate dependency
finding covered GHSA-4jqv-mc3x-m676 and GHSA-mcj8-r9mp-w47p. The manifest and lock now select 15.5.27;
three offline security checks passed after a real Red, including refusal to adopt the unsafe 16.0.0
branch merely because its major version is higher. Historical 0.5.3 audit results remain unchanged;
dependency patching does not update or prove exploitability of a running deployment.

Separately, **four task-navigation browser cases passed** on real production Next.js 15.5.27 pages
and Monaco using a fixture-only Engine URL: same-page draft preservation, expired-Session recheck,
pending logout and fragment-preserving login redirects. Engine replies were mocked and unapproved
network requests blocked; these are neither real server saving nor cache-poisoning exploit acceptance.
The temporary loopback frontend was stopped. Remote CI, Rust advisory audit, deployed restart,
backup replay, kernel, SSH and LAN acceptance were not rerun.

<!-- original-project-status:end -->
