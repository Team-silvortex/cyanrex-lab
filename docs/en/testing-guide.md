# Current project testing guide

[Format 3 marker declarations](platform-marker-expectation.md) add three pure value tests, nine
strict input tests and five opt-in SQL selections in `bash scripts/test-platform-marker-expectation.sh`.
They verify declared identities/current-diagnostic compatibility, not deployed host or LAN security.

The **2026-10-16 Unreleased** [bound Session connection](platform-session-connection.md) adds
`platform_session_backend_tdd` transport regressions and the explicit
`bash scripts/test-platform-session-connection.sh` database/role/lock/trigger runner. Use only a
disposable `CYANREX_TEST_DATABASE_URL`; these tests neither mount a host nor issue public Sessions.

The Unreleased [local text-file import](local-text-file-import.md) has 12 pure tests, 6 browser
cases and 5 structural guards. Use `test:editor-languages` and `test:task-text-import-browser`;
controlled browser identity does not prove server saving or deployment.

Architecture ownership is indexed in the [current architecture map](architecture-map.md).
`scripts/tests/architectureMap.test.mjs` checks all 44 service declarations, visibility/Unix gates,
unmounted composition and bilingual navigation. These are structural guards; compiled file-error
matrices and explicit Source/HTTPS runners validate their respective behavior separately.

Source release **0.5.8** (**2026-10-13**) includes the dated follow-ups and their test selections.
`Unreleased` labels retain the original implementation stage; inventories and earlier 0.5.4 execution
records are not fresh 0.5.8 SQL runs or deployment acceptance. Actual release-gate results and omissions
belong in [project status](project-status.md).

The [ordered download](large-file-download-client.md), [manifest HTTP](large-file-manifest-http.md)
and [exact manifest client](large-file-manifest-client.md), selected saving, observation/cancellation
and paired distribution router are now included in 0.5.8; dated 0.5.7 evidence stays separate.

Reviewed against **0.5.4 on 2026-10-08**, including C2-T–W intent, dispatch and inspection boundaries,
the Next.js 15.5.27 floor, and earlier AI adapters, draft preparation, sanitizer hardening,
authentication guards and the standalone content HTTP adapter. This guide explains what to run for each module and boundary, what each layer
proves, and how to avoid using live data. It is a test plan and source inventory, not a new test run.
Use the [platform network](platform-network.md) to identify connected and unconnected product paths.
The [capability tensor](capability-maturity.md) links each implementation slice to test sources and
dated evidence. Its common-script checks validate structure, references and bilingual score-table
consistency; they do not execute the referenced product tests or establish deployment maturity.

The **2026-10-11 Unreleased multi-package follow-up**, still on source 0.5.4, extends metadata
composition tests and the two exact Session runners below. This is an inventory update; dated passing
results remain in project status, and the earlier review and evidence dates remain unchanged.

The separate **2026-10-11 Unreleased assessment-binding follow-up** adds paired nullable-definition
contract cases and one case to each of those runners. Both fields must be explicit, mixed bound/
unbound metadata keeps catalogue identity, and typed assessment remains bound-only. Current counts
below are inventory, not execution or old-binary compatibility evidence.

The separate **2026-10-11 Unreleased C2-Y follow-up** adds an explicitly configured catalogue
Task-content HTTP router. Its default, exact SQL and common-guard selections are listed separately
below; neither this inventory nor earlier manual-router results establish its execution or deployment.

The independent **2026-10-11 Unreleased C2-Z follow-up** adds a typed catalogue draft publication
attempt, separate from the manual C2-P–W workflow. Its preparation, controlled confirmations and
exact SQL selections below are source inventory, not inherited manual-publication acceptance.

The independent **2026-10-11 Unreleased C2-AA follow-up** adds read-only observation of one unknown
catalogue attempt target. Its current-read comparisons and typed failures are separate from C2-Z
publication and old manual C2-Q observation; neither earlier result set establishes this execution.

The independent **2026-10-11 Unreleased [task-content index](task-content-index.md)**, on source
0.5.5, adds `task_content_index_tdd` and `session_task_content_index_tdd`. The separate exact runner
`scripts/test-task-content-index.sh` selects **four trusted-store + eight Session SQL cases**, **12**
in total. This is source inventory, not execution or inherited release/content-reader acceptance.
Select pagination/cursor limits, manual/neutral catalogue admission, full record/lookahead validation,
private ownership, Artifact independence, current lifecycle and observed lock-wait cancellation/expiry.

The separate same-day **Unreleased [index HTTP adapter](task-content-index-http.md)** on source
0.5.5 adds `task_content_index_http_tdd` and **eight SQL source selections**, separate from the
12 service-index cases. Check strict raw queries, private projection, unchanged old routers, current
Session faults/waits and response-body capacity. This inventory does not establish execution,
public mounting, browser integration or cross-platform compilation. Its exact runner is
`scripts/test-task-content-index-http.sh`; source inventory/wiring guards are
`scripts/tests/taskContentIndexHttp.test.mjs`, not substitutes for database execution.

## Test layers

| Layer | Maintained entry | What a pass establishes | Not established |
|---|---|---|---|
| Common source and tooling checks | [`quality-gate.sh`](../../scripts/quality-gate.sh), [`scripts/tests`](../../scripts/tests) | File/version/course consistency, public API/SDK contract consistency, script regressions and synthetic tool fixtures | Deployed workflows, real SSH/Docker/kernel acceptance |
| Default Rust tests | `cargo test --manifest-path engine/Cargo.toml --locked` | Models, services, in-process routes, default integration fixtures and compile-time contracts | Explicitly ignored PostgreSQL, live Agent transport and performance cases |
| Explicit PostgreSQL tests | Named CI cases and storage runners below | Real SQL transactions, namespace/schema rules, locks, races, cancellation and injected failures in disposable data | Browser integration, deployed data migration or live authorization cutover |
| Frontend build and unit tests | Frontend quality gate and [`frontend/package.json`](../../frontend/package.json) | Production compilation, TypeScript, state/request/permission/editor regressions and fail-closed asset-generation checks | Real browser rendering or real Engine connectivity |
| Browser regressions | Explicit `test:*-browser` scripts | Real Next pages or local generated-asset fixtures, interaction, Monaco models/workers and fixture-defined request behavior | Real service authorization or end-to-end database/kernel flow when Engine responses are mocked |
| SDK and contract tests | `quality-gate.sh --sdk-only`, [`sdk-js/package.json`](../../sdk-js/package.json) | Generated models/operations, compatibility, runtime requests, type checks and packed consumer imports | APIs absent from the public contract, including prepared generic commands |
| Tool and release fixtures | `test-runner-agent-tools.sh`, `test-distribution-tools.sh`, `test-live-kernel-smoke.sh`, `test-release-candidate.sh` | Secret handling, target/metadata/archive validation, cleanup and error reporting under mocks | Actual installation, remote SSH changes, Agent deployment or kernel execution |
| Explicit live acceptance | `runner-agent-smoke.sh`, `live-kernel-smoke.sh`, `distribution-install-smoke.sh`, native `cyanrex-release` | Only the real environment, candidate and operations recorded by that run | Other kernels, machines, versions or isolation guarantees |
| Benchmarks | `bench-mainline.mjs`, `bench-event-stream.mjs`, `bench-event-history-reads.mjs` | The measured workload/profile/source snapshot | Correctness acceptance or an unmeasured whole-system throughput claim |

Every quality-gate mode runs the common preflight and tool fixtures first. Default mode then runs
backend, frontend and SDK checks. `--security` adds the Rust advisory audit; frontend/SDK checks also
run their production npm audits. `--format-only` is therefore not merely a formatter invocation.
The gate does **not** automatically run all ignored cases, browser suites or real privileged smoke tests.
The [CI workflow](../../.github/workflows/ci.yml) adds named PostgreSQL cases and real loopback Agent
compilation; consult its explicit lists rather than assuming `cargo test` covers them.

## Module and boundary selection

| Changed area | Primary tests | Adjacent boundary to include |
|---|---|---|
| Legacy authentication and roles | `routes_tdd`, `auth_service` unit/explicit SQL tests; `authSession`, `sidebarPermissions` frontend tests | Account lifecycle, CSRF, teacher authority, classroom enrollment cancellation, UI logout failure |
| Classroom discovery and invitations | `classroom_tdd`, `classroomConnection` tooling/unit/browser suites | Version/protocol/capability rejection, confirmed origin, one-time invitation → legacy account creation |
| Scripts and learning persistence | `module_boundaries_tdd`, `learning_store` tests | Owner isolation, corrupt/failed JSON writes, real script SQL, feedback revision races, historical resume without rerun |
| Modules, headers and terminal | `routes_tdd` module/command cases; header/dispatcher service tests; `terminalCommand` frontend tests | Teacher-only mutation, checksum-invalid downloads, selected metadata → compiler, no arbitrary shell dispatch |
| eBPF editor and execution | Compiler/loader/Runner tests; `compilerCheck`, `semanticCompletion`, runtime/breakpoint browser suites | Check versus run, compiler source freshness, cancellation/lease ownership, exact detach; live kernel acceptance remains separate |
| Runner Agent | Authenticator, registry, job queue/executor/client tests; `runner_agent_client_tdd`; inventory/admin browser tests | Signature/replay → lease/result, timeout/cancel, owner-bound remote check, no execution fallback |
| AI Agent profiles and SDK helpers | `ai_agent_settings_tdd`, private-store cancellation units, `test:ai-agents`, explicit `test:ai-agents-browser`, SDK Agent runtime/type/package tests | Teacher/CSRF → revision-fenced private metadata → exact reviewed UI acknowledgement; explicit tool selection, per-call approval, hostile calls and no automatic retry. Synthetic formats are not real provider acceptance |
| Teaching pack and generic catalogue | `task_catalog_tdd`, `teaching_task_adapter_tdd`, `collaboration_contract_tdd` | Typed evidence and exact definition/policy identity; catalogue admission does not execute rules or accept Tasks |
| Metadata-only catalogue composition | `task_definition_catalog/composition.rs`, `taskCatalogComposition` common guards, Session catalogue/content composition SQL cases | Explicit packages/versions, order, full-reference overlap and nested 256-definition global limits; existing schema 2/3 commands retain private ownership and full-definition checks, without providers or rule execution |
| Explicit assessment-binding choice | `task_definition_catalog/assessment_binding.rs`, `taskAssessmentBinding` common guards, Session catalogue/content binding SQL cases | Required nullable JSON fields, paired absence, old bound encoding, mixed metadata, typed-provider refusal before dispatch and both binding-drift directions; no manual-mode promotion, automatic Review or old-binary downgrade guarantee |
| Events and settings | `module_boundaries_tdd`, EventBus tests, explicit event SQL cases; event/settings unit and browser suites | Publication ordering → durable history → resync, export/delete filter safety, confirmed settings and read failures |
| Generic identity and authority | `collaboration_*_tdd`, `legacy_workspace_projection_tdd` | Audited binding/member/grant revisions, absent-key races, last manager, role versus deployment authority |
| Registry timestamps | `test-registry-timestamps.sh` selects ten cases from four existing targets | Nullable retirement validity, mandatory identity/policy audit times, finite limits, old history/replay, append readback, reconciliation errors and Session-command rejection; no new age/order policy |
| Durable source and lifecycle | `durable_auth_source_tdd`, `durable_session_boundary_tdd`, `durable_collaboration_tdd`, deletion/password/bootstrap/reconciliation targets | Source-only namespace/table pins → account incarnation → exact current Session → identity/policy on one transaction, namespace replacement, post-write facts |
| Prepared password work | Default `auth_service::durable_source::password_work` unit tests and source-wiring guard | Dispatched/queued capacity, cancellation/timeout lifetime, panic recovery and real hashing; rerun source, password-change and bootstrap SQL for all entry points |
| Missing-account password work | `test-durable-login-password.sh`, fixed-PHC unit and source/CI guards | Real shared-gate admission into a held executor, released SQL resources, cancellation, concurrent same-name registration and rejection even when the synthetic password matches; not a timing benchmark or complete enumeration defense |
| Explicit expired Session cleanup | `test-durable-session-cleanup.sh`: nine exact library SQL cases, default closed-pool test and source/CI guards | 128-row batches, active-session preservation, independent writers, lock-fresh cutoff, bounded text/time decoding, orphan/corrupt candidates and hit-proven deletion/commit/cancellation faults; no automatic or live-data cleanup |
| Prepared password profile | `test-durable-password-profile.sh`, default `password_profile`/`password_work` units and source/CI guards | Six exact SQL cases for unsupported records, writer compatibility, existing Sessions and rollback; hostile numeric costs are pure-preflight tests, never expensive test computations |
| Prepared OTP freshness | `test-durable-otp-freshness.sh` selects seven `--lib` SQL cases; default `otp::tests` and source/CI guards | Private clocks advance after observed SQL waits or snapshot release with the password executor held, not a proven queue-dispatch instant; rollback, success and freshness versus composed consumption. No machine clock changes or public clock override |
| OTP consumption policy | Default `otp_consumption::tests` and `durableOtpConsumption` source guards | Pure counter selection, real collisions, credential binding, exact final counter and input/time limits; pure units alone do not prove persistence/atomicity. The policy is now used by prepared login/rotation |
| Atomic OTP consumption | `test-durable-otp-consumption.sh`: 11 exact library SQL cases | Independent sources, reopen/logout, login/rotation competition, account incarnations, version/shape rejection, suppression/tampering/deferred failure/cancellation and real collisions |
| OTP source format | `test-durable-otp-schema.sh`: four exact SQL cases | Genuine schema 1 rejected unchanged, malformed schema-2 column/default/constraints refused, fresh registration/bootstrap at -1 and hit-proven initialization faults; no migration test or claim |
| Operator provisioning | `provision_cli_tdd`, `provision_postgres_tdd` | Read-only plan, target-bound apply, private enrollment delivery, lost acknowledgement and cancellation; no live configuration adoption |
| Generic Task, Artifact and Review | Explicit storage runners below; their default Rust tests | Exact revisions/digests, Task/outbox atomicity, immutable file boundaries, private ownership, Review history, current Session checks |
| Local task payload and editor | `test:editor-languages`, `test:task-payload-browser`, `test:multi-language-editor-browser` | Optional payloads, accepted content/revisions, import/export, stale edits, model disposal and forbidden network writes |
| DOMPurify and generated Monaco assets | `frontendDependencySecurity` common guards, default `test:tooling`, explicit `test:dompurify-browser` | Locked copies and overrides → exact vendor replacement → new chunk references → the sanitizer actually used by Monaco; dependency audit and deployed behavior remain separate |
| Internal Task content metadata | Rust `task_content_contract_tdd`, `task_content_binding_tdd` | Strict object shape, scalar/item limits, exact ordered pins, supplied owner/bytes/digest consistency; pure default tests, not storage or Session authorization |
| Large-file manifest and streaming core | Default `file_transfer_tdd`, private Unix source-check units and `fileTransfer` common inventory guards; [boundary](large-file-core.md) | Strict bounded metadata, real chunk/whole digests, exact EOF, fixed IO buffers, partial-output/IO faults and preopened regular-file checks; not private staging, durable progress, quota, authenticated transfer, benchmark or desktop interoperability |
| Private Unix chunk staging | Default `file_chunk_staging_tdd` and private queued-worker units; [boundary](large-file-staging.md) | Exact manifest binding, exclusive private files, retained failures, observable root/entry drift and actual cancellation/queue lifetime; not an upload receipt, reserved quota, real fsync/disk-full fault acceptance, recovery, transport or benchmark |
| Trusted upload intent and logical quotas | Default `file_upload_intent_tdd` admission cases, explicit `scripts/test-file-upload-intent.sh`, `fileUploadIntent` source guards; [boundary](large-file-upload-intent.md) | Fresh-only schema, exact owner/account generation, cross-generation byte/count competition, bounded corrupt-ledger rejection and real trigger/cancellation controls; metadata only, not chunk receipts, physical allocation, current Session, resume or transfer |
| Upload-bound local staging | Default `file_upload_staging_tdd` admission cases, explicit `scripts/test-file-upload-staging.sh`, `fileUploadStaging` source guards; [boundary](large-file-upload-staging.md) | Real cross-chunk bytes, original source/record checks after lock waits, original inode receipts, shared admission/cancellation and retained failures; no durable chunk journal, Session authority, resume, network, real disk-full/fsync fault or benchmark acceptance |
| Write-ahead chunk journal | Default `file_upload_chunk_journal_tdd`, explicit `scripts/test-file-upload-chunk-journal.sh`, `fileUploadChunkJournal` source guards; [boundary](large-file-chunk-journal.md) | Committed Unknown before any file dispatch, exact journal coordinates, genuine SQL waits/cancellation, retained files and metadata-only inspection; not recovered execution, current integrity from status, whole-file finalization, Session authority or transport |
| Whole-upload current-byte verification | Default `file_upload_whole_tdd`, explicit `scripts/test-file-upload-whole.sh`, private worker units and `fileUploadWhole` common guards; [boundary](large-file-whole-verification.md) | Ordered bound chunk/whole digests, original source/file snapshot drift, read-only results and shared worker admission; not historical receipts, journal adoption, durable publication, recovery, Session authority or transport |
| Persistent file publication metadata | Default `file_upload_publication_tdd`, explicit `scripts/test-file-upload-publication.sh`, `fileUploadPublication` common guards; [boundary](large-file-publication.md) | Exact complete confirmed journal and real whole bytes, immutable upload/version uniqueness, bounded global capacity, original pin and post-acknowledgement failures; not recovered receipts, permanent physical sealing, current Session, grants or transport |
| Session private file metadata | Default `session_file_metadata_tdd`, explicit `scripts/test-session-file-metadata.sh`, `sessionFileMetadata` common guards; [boundary](large-file-session-metadata.md) | Derived owner/account in one source transaction, exact private upload/Published reads, fresh policy/expiry, quotas, real waits/faults and original source/target pins; not actual Session publication, byte health, grants, recovery or transport |
| Pure file publisher/recipient grants | Default `file_access_tdd` and `fileAccess` common composition/document guards; [boundary](large-file-access-contract.md) | Exact account/version selectors, independent revocation, half-open time intervals and strict bounded JSON; not issuer provenance, audited current grants, Session/SQL authorization, byte access or transfer |
| Unreleased persistent publisher-grant metadata | `file_publisher_grant_tdd`, `session_file_publisher_grant_tdd`, explicit `scripts/test-file-publisher-grants.sh` and `scripts/test-session-file-publisher-grants.sh`, `filePublisherGrant` composition guards; [boundary](large-file-publisher-grants.md) | Trusted issuer/actor attribution, bounded full-ledger writes, CAS revocation, own current-Session reads and history-write fencing; not authenticated grant managers, publication permission consumption, recipient grants, bytes or live mounting |
| Unreleased host-configured Session grant management | `session_file_grant_management_tdd`, explicit `scripts/test-session-file-grant-management.sh`, `fileGrantManagement` guards; [boundary](large-file-grant-management.md) | Exact configured manager/account, derived actors, target lifecycle, deferred/original-pin/fresh Session checks; not inherited roles, durable online config revocation, publisher consumption, recipients, bytes or live mounting |
| Unreleased Session-authorized file publication | `session_file_publication_tdd`, explicit `scripts/test-session-file-publication.sh`, `fileSessionPublication` guards; [boundary](large-file-session-publication.md) | Exact current actor/grant, real bytes, original pins across the gap, fenced insert/deferred/fresh expiry and original post-ACK checks; not recipients, transport, live mounting, recovery or a lost-ACK rollback guarantee |
| Unreleased exact-file recipient ledger and own reads | `file_recipient_grant_tdd`, `session_file_recipient_grant_tdd`, explicit `scripts/test-file-recipient-grants.sh` and `scripts/test-session-file-recipient-grants.sh`, `fileRecipientGrant` guards; [boundary](large-file-recipient-grants.md) | Independent exact file/account bindings, CAS/history/quota and source-owned current recipient reads; not recipient management, download permission, bytes or transport |
| Unreleased publisher-owned sharing | `session_file_sharing_tdd`, explicit `scripts/test-session-file-sharing.sh`, `fileSharing` guards; [boundary](large-file-sharing.md) | Own exact Published selection, active publisher permission/current recipient, deferred original pins/deltas/final fresh expiry and independent original-owner revocation; not global managers, downloads, byte IO or transport |
| Unreleased recipient download manifest | `session_file_download_tdd`, explicit `scripts/test-session-file-download.sh`, `fileDownloadManifest` guards; [boundary](large-file-download-manifest.md) | Own Active recipient grant, exact granted publisher/Published version, original records and final same-clock grant/Session validity; metadata only, not byte health, reusable permission or transport |
| Unreleased authorized chunk snapshots | `session_file_chunk_read_tdd`, explicit `scripts/test-session-file-chunk-read.sh`, private file-read units and `fileChunkRead` guards; [boundary](large-file-chunk-read.md) | Actual bounded single-chunk bytes, original root/records across a lock-free gap, final recipient/Session expiry and retained budgets; not whole-file rehash, HTTP/client delivery or recovery |
| Unreleased unmounted binary chunk HTTP | `file_chunk_read_http_tdd`, explicit `scripts/test-file-chunk-http.sh`, private query/body units and `fileChunkHttp` guards; [boundary](large-file-chunk-http.md) | Strict empty GET, original current recipient command, fixed binary frames, both budgets through Bytes owners and first-frame deadline; not mounted LAN delivery, client save/resumption or recovery |
| Unreleased supplied-response Unix receiver | `file_chunk_receiver_tdd`, explicit `scripts/test-file-chunk-receiver.sh`, private job-lifetime units and `fileChunkReceiver` guards; [boundary](large-file-chunk-receiver.md) | Exact singleton head, bounded private frames/real EOF/hash and idle/total time before exclusive staging; not active fetching, remote receipts, whole-file assembly/save, resume or live mounting |
| Unreleased explicit single-chunk HTTP client | `file_chunk_client_tdd`, explicit `scripts/test-file-chunk-client.sh`, private request/job units and `fileChunkClient` guards; [boundary](large-file-chunk-client.md) | Real loopback request/files, exact inputs, separate header/body deadlines and retained job admission; ignored current-Source SQL and OpenSSL CA/SAN controls, not normal-app mounting or whole-file/resume acceptance |
| Unreleased local whole-file assembly | `file_chunk_assembly_tdd`, private staging job/drift units and `fileChunkAssembly` guards; [boundary](large-file-chunk-assembly.md) | Real legacy inputs, exclusive actual whole-file output/EOF/hash/readback, conflicts and cancellation ownership; no user-selected save, network scheduler, resumed progress or deployment acceptance |
| 0.5.8 ordered private download | `file_download_client_tdd`, private flow/job units, explicit `scripts/test-file-download-client.sh` and `fileDownloadClient` guards; [boundary](large-file-download-client.md) | Actual ordered HTTP-to-whole bytes, first-failure preservation and queued/running cancellation ownership; separate current-Source SQL bridge, no UI save/resume or mounted deployment |
| 0.5.8 recipient manifest HTTP | `file_download_manifest_http_tdd`, private contract units, explicit `scripts/test-file-download-manifest-http.sh` and `fileManifestHttp` guards; [boundary](large-file-manifest-http.md) | Strict six selectors/current recipient and bounded private JSON/body admission; no client fetching, bytes, mounted delivery or resumption |
| 0.5.8 exact manifest client | `file_manifest_client_tdd`, private framed-Body/job units, explicit `scripts/test-file-manifest-client.sh` and `fileManifestClient` guards; [boundary](large-file-manifest-client.md) | Real metadata GET/strict full-version check and ordered private whole bytes, including empty GET and actual Source revocation boundaries; no UI save/resume or mounted deployment |
| 0.5.8 host-selected private file saving | `file_save_tdd`, private source/output drift and job units, `fileSave` guards and the extended explicit manifest-client SQL bridge; [boundary](large-file-save.md) | Original descriptor/full-manifest continuity, exclusive actual copy/readback, preserved uncertain output and real worker lifetime; no ordinary-directory/UI save, resume or live mounting |
| Unreleased authorized download and private saving | `file_delivery_tdd`, private whole-flow/job lifecycle units, `fileDelivery` guards and `test-file-delivery.sh`; [boundary](large-file-delivery-client.md) | One admission across actual metadata/chunks/assembly/save, independent output readback and first-failure/cancellation preservation; not UI, resume or live mounting |
| Unreleased explicit owned user-directory saving | `file_user_save_tdd`, private directory/job/phase units, `fileUserSave` guards and extended existing delivery SQL cases; [boundary](large-file-user-save.md) | Fixed original parent UID/GID/mode, unsafe path/mode refusal, actual selected copy and cancellation; private staging stays 0700, no shared-writable-folder or UI acceptance |
| Unreleased live delivery progress observation | `file_delivery_progress_tdd`, private actual job lifecycle tests, `fileDeliveryProgress` guards and extended existing delivery SQL; [boundary](large-file-delivery-progress.md) | One latest snapshot, real ACK prefix, errors/cancellation/slow readers and actual output; not durable receipt, recovery, resume, UI or deployment acceptance |
| Unreleased explicit cancellation of delivery waiting | `file_delivery_cancel_tdd`, private real-job/channel-selection units, `fileDeliveryCancel` guards and extended existing Source cases; [boundary](large-file-delivery-cancellation.md) | Request versus handle-drop, input/cancel/capacity priority, real Cancelled with retained jobs/output; not instantaneous kill, rollback, recovery or UI |
| Unreleased complete HTTPS delivery verification | Three explicit native `file_delivery_tls_tdd` cases, `test-file-delivery-tls.sh` and `fileDeliveryTls` inventory guards; [scope](large-file-delivery-https.md) | Actual disposable TLS, complete saved bytes, peer refusal before credentials and explicit wait cancellation; separate from SQL authority or deployed-LAN acceptance |
| Unreleased paired distribution router | `file_distribution_http_tdd`, `fileDistributionHttp` guards and six explicit Source cases, including three combined native HTTPS cases in `test-file-delivery.sh`; [boundary](large-file-distribution-router.md) | One cached configuration, two independent HTTP gates, real binary owners/metadata/revocation; not a Session issuer or isolated service |
| Unreleased actual Source plus HTTPS delivery | Three `source::source_https` cases in `file_delivery_tdd`, the same `test-file-delivery.sh` and `fileSourceHttps` guards; [scope](large-file-source-https.md) | Actual current grants, original Published bytes, one router, native HTTPS, selected saving, pre-dispatch revocation and response-body cancellation; no deployed LAN, issuer or isolation |
| Draft import and publication plan | Rust `task_draft_publication_tdd`, `taskDraftPublication` common guards, frontend `taskDraft` shared fixtures | Strict bounded JSON, blank-title rejection, local identity removal, exact supplied-content mapping and canonical-export compatibility; no publication, current-Session check or browser saving |
| Session draft publication steps | Rust `session_task_draft_publication_tdd`, `scripts/test-session-task-draft-publication.sh`, `sessionTaskDraftPublication` common guards | Fixed allocation and Session binding, one write per advance, empty/multi-item success, confirmed prefix versus unconfirmed step, cancellation, revoked authority and final real-byte checks; no durable recovery or browser saving |
| Independent catalogue draft publication | `session_catalog_draft_publication_tdd`, internal `catalog_draft_publication::tests`, `scripts/test-session-catalog-draft-publication.sh`, `sessionCatalogDraftPublication` guards | No-I/O exact-definition freeze, independent token domain, one existing Session write per advance, full `Some` confirmation, retained prefix/uncertain target and manual/journal type separation; C2-AA comparison is separate, with no restoration, retry or HTTP/browser saving |
| Catalogue unconfirmed-target observation | `session_catalog_draft_observation_tdd`, internal `catalog_draft_publication::observation_tests`, `scripts/test-session-catalog-draft-observation.sh`, `sessionCatalogDraftObservation` guards | One existing current-Session read, full frozen `Some` and ordered bytes, valid differences versus typed corruption, missing/foreign target scope, cancellation and unchanged attempt; no adopted confirmation, historical commit/rollback inference, recovery or retry |
| Unconfirmed draft target observation | Rust `session_task_draft_observation_tdd`, `scripts/test-session-task-draft-observation.sh` | Original Session/state admission, exact read-only comparisons, typed errors, unchanged attempt, current Task bytes versus historical prefix and missing-Task scope; no rollback proof or resumption |
| Draft metadata checkpoints | Default Rust `draft_publication::tests::checkpoint`, `session_task_draft_checkpoint_tdd`, `sessionTaskDraftCheckpoint` common guards | Strict bounded metadata codec, ordered allocations/lengths, state/count relations and round trips; imported progress is data, not authorization, provenance, persistence or a restored attempt |
| Explicit checkpoint target inspection | Default `draft_publication::tests::inspection`, `session_task_draft_inspection_tdd`, `scripts/test-session-task-draft-inspection.sh`, `sessionTaskDraftInspection` common guards | Explicit scope/current Session, one existing reader, exact metadata/text checks, typed faults, missing/empty Task boundaries and unchanged data; no original identity/body provenance, journal or restoration |
| Immutable Session draft intent | `session_task_draft_intent_tdd`, `scripts/test-session-task-draft-intent.sh`, `sessionTaskDraftIntent` common guards | Real Ready/zero-progress admission, canonical bounded metadata, duplicate conflicts, exact owner/account generation, current source/registry/journal pins and failure outcomes; no resource dispatch, restored attempt or write-ahead execution |
| Journaled Session draft dispatch | `session_task_draft_dispatch_tdd`, `scripts/test-session-task-draft-dispatch.sh`, `sessionTaskDraftDispatch` common guards | Explicit schema 2, consumed pristine attempt, acknowledged Unknown reservation, exact owner/account/nonce gates, resource/marker atomicity and complete prefix/pin checks; no restart/retry or public/browser acceptance |
| Session draft journal inspection | `session_task_draft_journal_inspection_tdd`, `scripts/test-session-task-draft-journal-inspection.sh`, `sessionTaskDraftJournalInspection` common guards | Schema 2 only, current exact owner/account, complete committed prefix plus optional Unknown, final Task count, pins/fresh authority and typed failures; no resource/file read or recovery |
| Journal step resource observation | `session_task_draft_journal_observation_tdd`, `scripts/test-session-task-draft-journal-observation.sh`, `sessionTaskDraftJournalObservation` common guards | Ordinal admission, same-transaction exact resource reads, original full journal/nonces/first pins after the read, final fresh authority and body-free result; no business writes, receipt or recovery |
| Separate content storage | Rust `task_content_store_tdd`, `scripts/test-task-content-storage.sh` | Schema 2/3 isolation, complete metadata/outbox atomicity, revision conflicts and storage faults; not a Session content or browser adapter |
| Session task content | Rust `session_task_content_tdd`, `scripts/test-session-task-content.sh` | Current authorization, old/new exact text, metadata-only edits, removal, namespace pins and post-write checks on one transaction; no public/browser acceptance |
| Private task-content metadata index | `task_content_index_tdd`, `session_task_content_index_tdd`, `scripts/test-task-content-index.sh` | Bounded UUID pages, position-only cursor, strict heads/lookahead, explicit neutral catalogue admission and current Session without Artifact access; no complete inventory, content availability, HTTP/SDK/UI or recovery acceptance |
| Independent metadata index HTTP | `task_content_index_http_tdd`, `scripts/test-task-content-index-http.sh`, `taskContentIndexHttp` guards; [contract](task-content-index-http.md) | Exact GET transport, strict raw cursor/limit, private seven-field projection, current Session delegation, corruption/empty-column refusal, Artifact independence and response-lifetime capacity; no public/browser/SDK connection or cross-platform/deployment acceptance |
| Independent catalogue Task-content HTTP | `catalog_task_content_http_tdd`, `scripts/test-platform-catalog-task-content-http.sh`, `catalogTaskContentHttp` common guards | Strict exact-reference objects → trusted full definition, bound/unbound catalogue identity, manual/owner separation, current Session/full metadata/text checks and shared transport/admission outcomes; no evaluator or public/browser mounting |
| UI navigation and safety | `test:ui-permissions`, layout/action/account/runtime/browser suites | Target-bound confirmations, duplicate actions, route changes, keyboard access, keeping unsaved drafts |
| API, SDK, packaging and deployment | Common contract tests, SDK gate, release/SSH Rust targets and tool fixture runners | Engine route → OpenAPI → generated SDK; archive provenance → exact reviewed SSH target; fixtures versus real installation |

Names such as `authSession` identify the matching files in `frontend/tests`; exact npm script names
are listed in `frontend/package.json`. Use `--list` before an exact Rust filter: a mistyped filter can
otherwise report success with zero tests. The explicit storage runners enforce this check themselves.

## Safe default workflow

Run from a checkout intended for testing. Remove inherited deployment configuration before starting:
in particular `DATABASE_URL`, `CYANREX_*`, PostgreSQL connection variables and dotenv preload settings.
Then set only the fixture-specific values required by the chosen suite. Merely unsetting
`CYANREX_TEST_DATABASE_URL` does not protect a process whose normal services inherited `DATABASE_URL`.
Never reuse the deployment data directory, private Artifact root, credentials or Agent token.

After preparing that isolated environment, select the smallest relevant gate and its boundary tests:

```bash
./scripts/quality-gate.sh --backend-only
./scripts/quality-gate.sh --frontend-only --no-npm-install
./scripts/quality-gate.sh --sdk-only --no-npm-install
```

`--no-npm-install` is appropriate only when dependencies already match their lockfiles. Normal frontend
builds synchronize course copies from `docs/`; do not edit `frontend/public/course` as the source.
For documentation-only changes, check lengths, version synchronization, course synchronization and
links; do not describe those checks as a fresh runtime regression run.

The frontend gate also runs `test:private-request` for the shared transport lifecycle. Feature tests
retain their response/acknowledgement policies; explicit component-browser fixtures check Settings,
Metrics, Runner, Events and Runtime state separately. A cancelled browser wait is not server rollback.

## Explicit database suites

Use a newly provisioned disposable PostgreSQL instance with a dedicated test database and private
temporary files. A private Unix socket or restricted loopback listener is suitable; never point these
suites at a deployed database. Fixtures deliberately create/drop schemas, replace records, introduce
triggers and corrupt synthetic content. Some need schema/object privileges unavailable to application
roles. Use the isolation procedure in [acceptance](acceptance.md) and the current CI fixture as examples,
not as permission to reuse old local database credentials or paths.

Set `CYANREX_TEST_DATABASE_URL` explicitly to that disposable database. Keep `DATABASE_URL` unset.
Some legacy SQL suites also require `CYANREX_DB_FALLBACK=true`; this is a fixture setting, not an
authorization to accept fallback as successful persistence. Provisioning SQL fixtures additionally use
`CYANREX_TEST_DATABASE_PASSWORD`. Follow each exact suite's fixture and CI environment.

| Runner | Cases in this source snapshot | Main boundary |
|---|---:|---|
| `scripts/test-task-storage.sh` | 15 | Trusted Task storage, lifecycle revisions and outbox |
| `scripts/test-task-content-storage.sh` | 21 | Separate schema 3 manual Task/manifest persistence, atomic edits, legacy rejection and fault/concurrency checks |
| `scripts/test-artifact-storage.sh` | 17 | Exact immutable content, private file faults and pinned inputs |
| `scripts/test-review-storage.sh` | 17 | Review history, human/rule separation and non-teaching integration |
| `scripts/test-session-task-storage.sh` | 17 | Current Session → private manual Task |
| `scripts/test-session-artifact-storage.sh` | 18 | Current Session → private publication and exact read |
| `scripts/test-session-task-inputs.sh` | 20 | Task → exact owned Artifact inputs and post-write verification |
| `scripts/test-session-review-storage.sh` | 18 | Session → private human Review with exact target/evidence |
| `scripts/test-session-catalog-tasks.sh` | 20 | Exact admitted bound/unbound definition metadata, provider-free single/composed catalogues and 0–32 inputs |
| `scripts/test-session-task-revisions.sh` | 17 | Draft replacement, old/new input union, schema 2 compatibility and competing saves |
| `scripts/test-session-task-content.sh` | 33 | Schema 3 manual/catalogue content, bound/unbound composed metadata, exact definitions/text, old/new rechecks, namespace substitution, lifecycle and concurrent faults |
| `scripts/test-platform-task-content-http.sh` | 13 | Standalone HTTP → C2-M, private ownership, Session expiry during lock waits, exact text, commit/outbox faults, competing edits and shared admission through response-body completion |

These twelve runners total **226 selected cases** (159 preceding resource cases, 21 content-store, 33 Session-content and 13 standalone HTTP cases),
not all PostgreSQL coverage in the repository.
The two catalogue additions cover metadata-only construction and exact admission without an evaluator;
this is current source inventory, not a revision of earlier execution counts.
The six catalogue-content additions cover explicit mode separation, full-definition consistency,
text/revision guards and current authorization around faults and waits, using the existing schema 3.
The multi-package follow-up adds one case each in `session_catalog_task/composition.rs` and
`session_task_content/catalog_composition.rs`. Select these through the maintained exact runners;
pure composition checks or source guards do not replace their database execution.
The separate assessment-binding follow-up adds `session_catalog_task/assessment_binding.rs` and
`session_task_content/catalog_assessment_binding.rs`. Check bound/unbound definitions through the
existing schema 2/3 paths, explicit null persistence, unchanged private content and rejection of
adding/removing bindings under an existing reference. The two cases are included in the inventory
above, not additional cases beyond their exact runners.

For the **2026-10-08 Unreleased C2-N admission follow-up**, check the shared capacity across cloned
states and separately constructed routers, refusal before body polling or SQL, and slot retention
through successful and error response bodies. Cover collection/cancellation, terminal-frame/end/drop
release and unchanged private error outcomes. Explicit SQL regressions should include a retained
successful content body and a waiting mutation, overflow without business effects, cancellation and
the original Session checks. The current HTTP inventory adds two exact SQL cases to the original eleven;
the inventory is not a completed-run claim. These are adapter boundaries, not a global worker quota or rollback proof;
the original 0.5.0 results are not execution evidence for this follow-up.

The separate **2026-10-08 Unreleased C2-X** target `text_artifact_http_tdd` and exact runner
`scripts/test-platform-text-artifact-http.sh` cover text publication and exact-revision projection,
with ten exact SQL cases separate from the twelve-runner inventory above. Its twelve default cases
and three new common guards are separate selections. Check the independent 2 MiB wire / 256 KiB decoded-text limits,
private errors and current authorization, historical revisions, response-body slot retention and
independent text/Task budgets. The generic reader may materialize 1 MiB before projection; C2-N's
64 KiB request limit stays unchanged. Taking a returned reference into a separate Task request is a
caller-controlled in-process fixture sequence, not an atomic batch, router merge or browser test.
Source guards and default transport tests do not replace explicit disposable-database execution;
actual dated results belong in [project status](project-status.md).

The separate **2026-10-11 Unreleased C2-Y** target `catalog_task_content_http_tdd` has **28 default
cases** and **eight exact SQL cases** selected by `scripts/test-platform-catalog-task-content-http.sh`,
outside the twelve-runner inventory above. The defaults comprise one missing-reference case,
three exact-reference cases, eight catalogue-body cases and reuse of six transport plus ten admission
cases against the actual catalogue router. Three `catalogTaskContentHttp` common guards check source
boundaries and the exact inventory; these are selections, not passing-run claims.

The SQL cases cover all four commands with composed package/version and assessment-binding metadata,
bidirectional manual/catalogue/owner privacy, complete-definition drift and unknown coordinates,
old/new bytes, current Session and account lifecycle, post-write/commit faults, observed lock waits,
and response-body/cancellation capacity. Rejection snapshots compare all rows in **seven Task/Artifact
resource tables** and all private-directory file names/bytes, not every auth/identity/audit table.
The router retains C2-N's 64 KiB / two-second body boundary and uses its own typed state family;
separately configured manual/catalogue/text budgets add rather than forming a global quota.
Unknown definition is generic `400 invalid_request / unconfirmed` after a Session command was called,
not an HTTP rollback guarantee. Neither source/default checks nor older manual SQL results replace
this adapter's disposable-database execution. It remains unmounted, without automatic router merge,
catalogue discovery, client mode selection, provider assessment, journal P–W integration or browser saving.
See [the C2-Y contract](catalog-task-content-http.md) and dated [project status](project-status.md).

0.5.3 C2-P adds a separate `scripts/test-session-task-draft-publication.sh` runner for explicit
12 create-only workflow cases, not included in the twelve-runner total. It selects exact cases from
`session_task_draft_publication_tdd` against disposable storage; default preparation/state tests do
not substitute for those SQL runs. Dated execution results remain in [project status](project-status.md).

The independent **2026-10-11 Unreleased C2-Z** target `session_catalog_draft_publication_tdd` selects
**four default preparation cases**; the internal `catalog_draft_publication/tests.rs` selects **eight
controlled receipt/cancellation cases**. Six `sessionCatalogDraftPublication` guards check the opaque
type, two shared pure helpers, source wiring and exact inventory. None of these selections is a new
passing-run claim or a substitute for database/file execution.

`scripts/test-session-catalog-draft-publication.sh` selects **eight distinct exact SQL cases**:
behavior two, faults three, lifecycle one and waits two, outside the twelve-runner total above.
They cover composed packages/versions and bound/unbound definitions, ordered/empty plans, original-Session
substitution, effective fault counters, complete definition/manifest confirmation, damaged confirmed
bytes, preallocated Artifact/Task conflicts without adoption and current logout/expiry/membership
at each step. Wait fixtures must establish the PostgreSQL blocker before testing cancellation or
database-clock Session expiry at both Artifact and final Task steps.

Snapshot comparisons cover all rows in seven Task/Artifact resource tables and private-directory
file names/bytes, not every auth/identity table. Distinguish unchanged rejection snapshots from the
failed second-Artifact fixture's exact new orphan blob: the earlier committed prefix remains, SQL
does not adopt the failed target, and the unknown blob is neither deleted nor retried. The frozen
definition and `Some` confirmation checks do not authenticate preparation, assess a provider or
make the old manual checkpoint/journal/observation APIs accept this attempt. See [the C2-Z contract](session-catalog-task-draft-publication.md);
actual results and omissions remain separately dated in [project status](project-status.md).

The independent **2026-10-11 Unreleased C2-AA** target `session_catalog_draft_observation_tdd` selects
**four default cases** for Ready/original-token admission, exact Artifact and empty-Task reader errors,
unpolled observations and unchanged progress. Internal `catalog_draft_publication/observation_tests.rs`
selects **six controlled comparison/admission cases**; supplied-value corruption checks there are not
real-reader acceptance. Six structural guards check current-reader delegation, shared body-free DTO,
no state adoption and exact inventory, not database/file execution.

`scripts/test-session-catalog-draft-observation.sh` selects **eight distinct exact SQL cases**:
behavior three, faults two, lifecycle one and concurrency two, outside the twelve-runner total above.
They distinguish valid registered-definition/status/content changes from effective same-pin metadata,
manual `None` and both-direction binding corruption; typed `InvalidRecord` must not become a report.
They cover composed versions, bound/unbound and ordered/empty content, current Session/membership,
damaged or missing bytes, absent/foreign targets and the missing-Task Artifact-namespace early return.
Wait cases must prove actual PostgreSQL blocking before cancellation or database-clock expiry.

Observation-preservation snapshots compare every table's sorted-row MD5 in the selected source,
Artifact and Task schemas, plus private file names, lengths, modes and SHA-256 and root device/inode/mode.
These are scoped summaries, not raw-row/byte equality or an audit of every deployment schema. Fault
fixtures prove injection counters/effective stored mutations before observing. An Artifact SQL rollback
can leave its exact immutable blob, and the resource counts are measured separately; `NotVisible`
never proves rollback. Pending-creator cases use a separately explicit command at an observed outbox
barrier, not a fabricated lost acknowledgement, and later visibility never confirms the attempt's
earlier write. See [the C2-AA contract](session-catalog-task-draft-observation.md) and separately dated
[project status](project-status.md) for actual results and omissions.

0.5.3 C2-Q has a separate `scripts/test-session-task-draft-observation.sh` exact runner and
`session_task_draft_observation_tdd` target. Its 12 read-only cases are separate from C2-P’s 12 creation
cases: observations must preserve attempt state and distinguish current visibility from past commit
confirmation. A missing Task does not require inspecting the Artifact namespace; read errors must not
be converted into absence. These cases are outside the twelve-runner total above.

0.5.3 C2-R checkpoint tests are default pure-contract/library checks, not another PostgreSQL runner.
They must reject malformed/oversized documents and inconsistent targets or progress while keeping
parsed data separate from live attempts. Round trips do not establish crash durability or permit
calling C2-Q after restart; storage and recovery require separate future tests.

0.5.3 C2-S uses `scripts/test-session-task-draft-inspection.sh` for the independent inspection
entry point. It supplies current Session authority rather than recreating C2-Q's original attempt.
Select its exact database cases separately from the C2-R codec and C2-Q observation suites; metadata
agreement must never be counted as historical confirmation, original-body comparison or safe retry.

0.5.4 C2-T uses `scripts/test-session-task-draft-intent.sh` to select registration/read cases
from `session_task_draft_intent_tdd` on an explicitly installed disposable journal. Keep its fresh
schema/installation-ID, exact account-generation, duplicate/conflict and transaction-fault checks
separate from C2-P writes. Source/registry/journal validation does not test original Task/Artifact
incarnations. A confirmed record is not a dispatched resource or proof of durable-before-dispatch;
database COMMIT settings do not substitute for host recovery or backup-replay tests. Dated execution
results belong in [project status](project-status.md), not the earlier runner totals above.

0.5.4 C2-U uses `scripts/test-session-task-draft-dispatch.sh` with
`session_task_draft_dispatch_tdd`. Check explicit schema 2 admission without upgrading schema 1,
confirmed A-before-B ordering, authorization changes, exact nonces/prefixes, marker triggers before
business validation and final read-only journal checks. Resource SQL and its marker must share B's
commit; an uncertain acknowledgement must not become rollback proof or retry permission. Existing
C2-T registration/read cases remain separate and do not establish this dispatch behavior.

0.5.4 C2-V uses `scripts/test-session-task-draft-journal-inspection.sh` with
`session_task_draft_journal_inspection_tdd` for ten selected SQL cases; two default tests and three
common guards are separate. Dated execution results are recorded in [project status](project-status.md). Check legal
prefixes, final Task counting even for an empty payload, exact owner/account visibility, schema/pin
faults, wait expiry and cancellation without resource or file access. Missing records do not prove
rollback, and recorded progress cannot restore a wrapper or permit retry. C2-T/U evidence stays separate.

0.5.4 C2-W selects twelve SQL cases through `scripts/test-session-task-draft-journal-observation.sh`
and `session_task_draft_journal_observation_tdd`, with two default cases and three common guards.
Dated execution is recorded in [project status](project-status.md). Check pure ordinal refusal, missing
intent versus unrecorded step versus invisible resource, old exact Artifact revisions, edited/current
Task contents, typed blob failures and the original complete journal/pin recheck after resource access.
Body-free output still reads files; waiting/cancellation must not grant receipts, mutation or retry.
External-writer blocking does not test mutation inside the read transaction; first-snapshot reuse is source-guarded.
C2-V remains resource-free, and its prior execution does not verify this separate read path.

Legacy auth/events/scripts/learning and generic identity/source/lifecycle/provisioning cases have
separate explicit lists in CI. The runner inventory is guarded by
[`postgresCi.test.mjs`](../../scripts/tests/postgresCi.test.mjs).

The separate `scripts/test-prepared-resource-sql.sh` runner selects two additional library cases for
the shared Task/Artifact/Review mechanics. It checks ordinary-table rejection and decode order, plus
read/write transaction settings and LOCAL timeout cleanup after rollback/drop on one pooled connection.
Its exact inventory and CI inclusion are checked by `architectureBoundaries.test.mjs`; these two cases
are not included in the twelve-runner total above and do not replace store or Session coverage.

As of the 2026-10-07 expiry-decoding follow-up, `scripts/test-durable-session-boundary.sh` selects
**17 source-only Session cases**, the original 14 plus three timestamp cases:
ambiguous/temporary namespaces, filtered or incompatible relations, post-write redirects/replacements,
metadata changes, login's two-transaction identity, logout/expiry lock ordering, and invalid/boundary
Session expiry through validation, login writeback and the password-change guard. It needs no
collaboration registry and is guarded by `durableSessionBoundaryCi.test.mjs`. It is separate from the
212 resource cases and the original 14 `durable_auth_source_tdd` SQL cases. These inventory counts are
not claims that every suite was rerun; see dated project status.

`scripts/test-durable-reconciliation.sh` now selects **17 cases**: the original twelve lifecycle checks,
two Session-expiry cases and three subsequent registry-timestamp cases from 2026-10-07. Session checks
retrieve only an SQL expiry boolean; registry cases cover precise error stages, finite audit times and
denied Session-authorized binding. The runner checks exact names before execution. These are inventory
counts, not new passing-run evidence; historical C1-K results are unchanged.

The 2026-10-07 registry follow-up adds **10 distinct SQL cases**: two identity-store, two identity-audit,
three policy-audit and three reconciliation cases. The corresponding full inventories are **17, 18, 19
and 17** cases. `scripts/test-registry-timestamps.sh` is a focused exact selector for those ten, not ten
additional cases beyond those suites; CI includes them through the maintained full-suite selections.
Range guards protect the three stored registry columns, not arbitrary timestamps or wall-clock ordering.

For example, with the disposable URL already set:

```bash
bash scripts/test-session-task-revisions.sh
```

Record each executed suite, not just an aggregate. Stop the exact temporary PostgreSQL instance and
remove only its validated fixture directory after the run. A failed or cancelled command does not
prove transaction rollback; inspect the test's assertions and retained evidence. Likewise, a failed
Artifact publication or Task replacement is not permission to delete retained content automatically.

## Browser and transport suites

For Next-backed page suites, use a **production frontend build** with a fixture-only `NEXT_PUBLIC_ENGINE_URL`. At browser-test time,
set matching `CYANREX_UI_ENGINE_URL` and the temporary frontend's `CYANREX_UI_BASE_URL`; configure
`CYANREX_PLAYWRIGHT_MODULE` and `CYANREX_CHROMIUM_PATH` if not available at their defaults. Inspect each
suite's helper before running: environment values and fixtures are not a connection to a real backend.
Production mode matters because some WebSocket fixtures would interfere with development HMR.

For the local payload/editor boundary:

```bash
npm --prefix frontend run test:task-payload-browser
npm --prefix frontend run test:multi-language-editor-browser
npm --prefix frontend run test:editor-intelligence-browser
```

The payload helper uses real pages and Monaco, permits only mocked identity/unread reads, and rejects
payload network writes and WebSockets. This is useful proof of local-only behavior, not a server-save
test. Other browser suites mock Engine routes for auth, classroom, learning, events, settings, Runner
administration and safety scenarios. No currently implemented browser suite can prove the absent
generic Task HTTP save workflow.

### Generated Monaco sanitizer fixture in 0.5.3

The DOMPurify follow-up adds default `test:tooling` units for the asset-generation boundary and
extends the common offline dependency guards to the DOMPurify 3.4.16 floor, alongside sharp and
source-map-js. Units check supported versions, original chunk hash and syntax shape, isolated official
ESM wrapping, unchanged bytes outside the vendor block, deterministic new chunk URLs and rewritten
references. Unknown input fails closed; the tests do not authorize patching `node_modules` or upgrading
Monaco silently. Live production npm audit remains separate, with its existing threshold.
Tooling units also inject preparation and patch-write failures to check private staging outside
`public` and preservation of existing assets before publication; they do not claim atomic publication.
The common Docker-input guard also requires dependency-stage generated assets and excludes the host's
Monaco asset directory from the build context; this source check is not a Docker image build.

With matching installed dependencies and Playwright/Chromium available, regenerate the local assets
before selecting the browser fixture from the repository root:

```bash
node frontend/scripts/sync-monaco-assets.mjs
npm --prefix frontend run test:dompurify-browser
```

This explicit suite contains ten cases. Nine instrument the actual generated AMD chunk to inspect the
same DOMPurify instance used by Monaco, not merely the root npm package. They cover the patched
version, safe formatting and executable-markup removal, before/after-element and after-attribute
removal hooks including kept custom elements, rejected rawtext roots, healthy root identity, and
real Monaco hover rendering. Test instrumentation is not shipped in generated production assets.

The tenth case loads the unmodified AMD loader and editor entry, new hashed chunk and language-registration
graph. Requests to the synthetic `monaco-fixture.invalid` origin are intercepted and fulfilled only from
local generated files; unexpected origins and paths are rejected. The first nine cases reject every
network request. No case makes external network contact or starts a Next or Engine service.
The suite accepts the same optional Playwright/Chromium path settings above but needs
no production frontend build or Engine URL. These are component/browser regressions, not proof of
application exploitability, a deployed sanitizer version or end-to-end acceptance. Browser execution
is opt-in; the default frontend gate runs the pure tooling units, not these ten browser cases.
The suite inventory is not a passing result; dated outcomes remain in [project status](project-status.md).

Real transport checks are separate. The ignored `runner_agent_client_tdd` case starts a loopback HTTP
server and uses Linux Clang with the BPF target; it compiles without attaching a program. The event
stream pressure harness checks real paced, overloaded and stalled sockets. Neither proves a real
LAN browser/teacher/student workflow, TLS deployment or VM isolation.

## Live and release acceptance

`test-live-kernel-smoke.sh` uses mocks; **`live-kernel-smoke.sh` is different and performs real privileged
work**. Run the latter only on an explicitly designated disposable Linux stack after reviewing its
target, credentials, empty attachment requirement and cleanup. It binds an observed kernel event to
its program, detaches the exact pin and rejects residue. Candidate evidence must match the package
metadata, revision and image content it claims to validate.

Likewise, `test-distribution-tools.sh` is not an installation, while `distribution-install-smoke.sh`
can start a real stack. SSH fixture tests do not authorize a remote apply. See
[offline deployment and release acceptance](../../docker/README-DEPLOY.md), [acceptance](acceptance.md) and
[classroom connection](classroom-connection.md) before those operations. Keep real privileged runs,
mock tool checks, dependency advisory scans and benchmarks as separate evidence categories.

## Evidence and maintenance

The unreleased **C2-L storage run** passed 436 default Rust cases (416 ignored), 111 common script
checks and, separately, 176 exact PostgreSQL cases: 21 new plus the preceding 155 resource cases.
The database was a fresh private-socket PostgreSQL 16 instance, not a deployed service. No content
Session/browser adapter or migration was exercised; see the dated cleanup and scope in project status.

The unreleased **C2-K pure-contract run** passed 428 default Rust cases (395 ignored) and 110 common
script checks, including 26 new metadata/snapshot cases. It did not rerun PostgreSQL or browser tests.

The earlier **C2-J backend development run** passed 402 default Rust cases, 92 common script
cases and the preceding 155 resource PostgreSQL cases. The earlier **task payload/editor run** passed
164 default frontend tests and 35 browser cases. These were separate scoped runs recorded on
2026-10-03; they must not be added into a single fresh end-to-end acceptance claim. The backend run
did not rerun every durable-source or lifecycle database suite, and browser mocks do not prove live
server saving. See [project status](project-status.md) for exact qualifications and previous phases.

For every new run, retain date, source revision/worktree state, selected tests, exact environment,
passed/failed/ignored counts, fixture boundaries and cleanup status. Never count a repeated case as
new coverage. Preserve [the historical test network](testing-network.md) and its reports rather than
rewriting old failures, counts or source fingerprints. Update this guide when a runner, boundary or
test environment changes, and keep its [Chinese version](../zh-CN/testing-guide.md) aligned.
