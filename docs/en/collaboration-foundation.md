# ADR-001: Collaboration foundation and legacy baseline

Date: **2026-09-27**. Status: **accepted for C-M1 contracts and offline comparison only**.
Source baseline: `feec8494189b6ee22f20bbd8737fe190df3b22f3`, product **0.4.3**.
Included in source release **0.4.4**; the frozen baseline and historical verification below are unchanged.

This is the first implementation slice of the [next architecture](../zh-CN/next-architecture.md).
The product keeps its existing version sequence; contract version `1` does not reset it to `0.0.1`.
The [current architecture](architecture.md) remains the authority for deployed behavior.

Follow-up: [ADR-002 / C1-A](collaboration-identity-store.md) adds an independent, explicit PostgreSQL
identity registry. The C-M1 scope and verification below record the earlier slice, not a live migration.

## 1. Scope and evidence

C0 source inventory and C-M1 now have Rust contracts, an offline legacy permission preview, and
synthetic regression cases. They do **not** add routes, tables, startup migrations, a grant store,
login changes, an event dispatcher or a second writer. AppState and the existing route guards do not
call the new preview. All IDs in tests are synthetic, not exported user identities.

This source inventory is **not** a live-deployment inventory. No account/configuration secrets,
student submissions or database contents were read. The actual active storage of each running
instance, fallback history, backup restoration, account incarnations and live kernel resources must
be established separately before C-M2 or any cutover. C0 as a whole is not yet complete.

Implementation:

- [Contracts](../../engine/src/models/collaboration/mod.rs): identity, references, scalars and envelope.
- [Legacy preview](../../engine/src/services/legacy_workspace.rs): pure, fixed-scope comparison.
- [Contract tests](../../engine/tests/collaboration_contract_tdd.rs) and
  [permission projection tests](../../engine/tests/legacy_workspace_projection_tdd.rs).

## 2. Frozen source inventory

The following facts were checked against the baseline commit. A configured database URL is not proof
that a particular service has stayed on PostgreSQL: fallback is per service and may latch until restart.

| Area | Existing identity/storage authority | Constraint on the new core |
|---|---|---|
| Users/sessions | SQL `users.username` primary key; sessions reference username and store token digests. Explicit memory mode and some auth fallback remain. | Preserve password/TOTP/session behavior; do not mint a new Principal on every login. Durable logout/password/delete confirmation is not replaced by a volatile grant store. |
| Teacher/student | AuthService derives authority from the deployment account and server teacher/legacy-admin allowlists. `admin` is compatible teacher authority, not a second level. | Snapshot server-resolved roles, never browser fields or Agent registration claims. |
| Scripts | Owner is username; SQL `user_scripts`, or per-user JSON under the instance data root. The `default` instance has the historical `scripts/<username>.json` path; others use `scripts/<instance>/<username>.json`. | Retain private ownership and record the selected source. Do not choose SQL versus file by modification time. |
| Attempts/feedback | SQL `learning_attempts`, or `learning/<instance>/attempts.json`; feedback is the current reviewer/comment/revision/time on each attempt. | Preserve source bytes/digest, original IDs, ownership, stage and feedback revision. Missing historical feedback versions stay unavailable. |
| Progress | Per username/lab projection; earliest successful completion persists even after later failed attempts. | Keep this teaching acceptance policy; do not infer completion from the latest Run alone. |
| Events | Per-user bounded EventBus, optional asynchronous SQL persistence and volatile fallback. Old wire events have no durable event ID. | Retain as telemetry; never promote them into authenticated audit facts or use them as an outbox. |
| Headers/compiler | Header files and selection metadata, local toolchain/runtime configuration. | Inventory exact files/toolchain separately; local paths are not remote input bundles. |
| Runner/Agent/modules | In-memory leases, Agent registry/job queue and module state; real kernel attachments may outlive requests. | Restart does not prove cleanup. Keep running-resource ownership separate from future durable Run facts. A Runner Agent becomes a machine, not an AI participant. |
| Discovery/deployment | Name-bound ephemeral classroom invitations; native SSH operator workflow; instance-local teacher management. | Discovery does not authenticate. New workspace ownership does not confer SSH or instance deployment authority. |

Pinned implementation evidence:
[authentication][auth], [route tiers][routes], [session guards][guards], [learning routes][learning],
[progress aggregation][progress], [scripts][scripts], [learning storage][learning-store],
[SQL schema templates][migrations] and [Runner boundary][runner].
The full legacy route/network inventory remains in the [functional network](functional-network.md);
its historical fingerprints are not rewritten by this work.

## 3. Decision: identities and scope

1. `AuthorityId` identifies a durable Cyanrex instance authority. It is **not** the existing sanitized
   `CYANREX_INSTANCE_ID` label, a hostname, a filesystem path, or proof of an HTTPS peer's identity.
   C1 must persist its allocation and bind it to verified instance configuration.
2. Principal, workspace, artifact, artifact revision, task, run, review, event and correlation IDs
   are distinct Rust newtypes. Their draft JSON representation is a non-nil, lowercase, hyphenated
   UUID. Reading/decoding never allocates IDs and rejects noncanonical spellings rather than merging them.
3. Principal kinds are `human`, `agent`, `service`, `machine`. Teacher/student are not identity kinds.
   A Principal has an explicit active/disabled status; Workspace has active/archived status, and
   Membership has active/suspended status. Role references name workspace-local presets.
4. A `PrincipalRef` carries authority plus principal ID; a `WorkspaceRef` carries authority plus
   workspace ID. Membership's principal ID belongs to the membership's authority. Cross-authority
   federation requires an explicit future binding, not comparison of local UUIDs or usernames.
5. `LegacyIdentityBinding` separates a canonical username from the supplied Principal ID. Current
   legacy usernames are lowercase ASCII, 3–64 bytes, using letters, digits, `_`, `-`, `.`. Import
   does not silently normalize names or recompute an ID from their spelling.
6. C1 must persist a unique active identity binding and retire it when an account is deleted. Reuse
   of a username must not inherit the old Principal's grants. Ambiguous recreated accounts or records
   without reliable owner provenance require reconciliation; C-M1 supplies neither that registry nor
   a claim that historical ownership can always be recovered.

## 4. Decision: legacy roles do not widen privacy

The preview takes **trusted, already-resolved** role, identity and resource ownership inputs. It never
reads environment variables or resolves a session itself. The fixed legacy Workspace must be selected
by the operator's stored mapping, not by a client-supplied workspace ID.

| Existing subject/action | Own legacy content | Another student's legacy content | Other workspace/instance |
|---|---|---|---|
| Any authenticated role: read private script/artifact | Allow | Deny | Deny |
| Any authenticated role: restore attempt using owner-only endpoint | Allow | Deny, including teachers | Deny |
| Student: teacher review operation | Deny | Deny | Deny |
| Teacher/legacy admin: teacher review operation | Only if the target is a student attempt | Allow student attempts, **not** arbitrary private artifacts or teacher/admin attempts | Deny |
| Teacher/legacy admin: deployment management | Explicit grant for the original instance | No grant to the student | Deny on another instance |

`student` projects to `cyanrex.teaching.learner`; `teacher` and legacy `admin` project to
`cyanrex.teaching.teacher` **plus a separate instance deployment grant**. The personal deployment
teacher retains teaching and deployment without switching accounts. Merely assigning
`cyanrex.workspace.owner` in a new workspace issues no such grant.

The preview compares four migration-sensitive operations only. A role name, serialized membership,
grant DTO or successful `permits` preview is not a credential. General resource grants, revocation,
disabled Principals, archived workspaces, dynamic role changes and policy storage belong to C1.
Live requests must still pass authentication, current role/resource lookups, CSRF and applicable
password/TOTP checks. A stale preview must never be cached as live authorization.

The tests cover teacher/admin/student mappings, owner-only restoration and private content,
student-only teaching review, another principal replaying a preview, equal IDs in another authority,
foreign workspaces and invalid action/resource pairs. They validate only this offline projection,
not deployed multi-workspace isolation.

## 5. Decision: pinned references and event records

Draft contracts use the existing Rust API's `snake_case` JSON convention. They are not yet part of
OpenAPI or the JavaScript SDK; their eventual public API version needs a separate decision.

- `ArtifactRef` requires workspace scope, artifact ID, **revision ID** and exactly 64 lowercase SHA-256
  hex characters. There is no floating `latest` reference. Equal content digests do not share ACLs.
  Decoding checks shape, not blob bytes, existence, ownership or authority; a publishing service must
  validate content and authorization before making the reference usable.
- `EventEnvelope` has independent `schema_version=1`, event ID, workspace, a typed aggregate reference,
  aggregate revision, optional actor/occurrence time, required recording time, namespaced event type,
  correlation ID, optional causation ID and optional pinned payload reference. Unknown historical facts
  stay unknown; recorded time is supplied by the recorder, never synthesized on deserialization.
- The envelope's workspace supplies the aggregate's scope. Application services must enforce referential
  consistency, actor authority and access to payload references before accepting a record. Structural
  decoding alone does not perform these checks or permit cross-space reads.
- Revisions are positive integers no larger than `9007199254740991`, preserving exact JSON values in
  JavaScript. Role/event names are qualified lowercase identifiers of at most 128 bytes, not commands.
- Unknown fields, unsupported envelope versions and unknown enum variants are rejected in these strict
  draft DTOs. Extension negotiation requires an explicit version/schema change; it is not silently lost
  during decode/re-encode. Existing public API unknown-field behavior is unchanged.
- The current telemetry `Event` is unchanged and cannot decode directly as an envelope. Importing
  identical historical telemetry rows must retain separate provenance/IDs; matching content is not
  proof of one event. No telemetry-to-business-event converter is introduced here.

This work does not implement outbox storage, ordering, stream offsets, replay, Run recovery or trusted
auditing. A future transaction must commit business state plus outbox together. A stream's committed
offset is separate from event ID and aggregate revision; none of these fields alone promise exactly-once
execution or cancellation rollback.

## 6. API baseline and single-writer decision

The protected baselines remain byte-for-byte unchanged:

| Baseline | SHA-256 at C0 |
|---|---|
| [OpenAPI compatibility](../../sdk-js/compatibility/openapi-baseline.json) | `59626ac51895120c9111683f2d4a9d95d03334edd250c2e8f72413fc448cfca6` |
| [SDK public surface](../../sdk-js/compatibility/public-surface.json) | `5ab9e5308b06c25b8c67b141301a62281f48a2fe845249d7fe34ca1bf41bcab9` |

Old namespace paths, operation IDs, responses and `staff`/`admin` access-category names remain.
The [SDK policy](../../sdk-js/STABILITY.md) applies additive-only patch rules to every minor line.
Its conservative retirement window is now unambiguous: a member deprecated in `0.4.x` remains for
**all of `0.5.x`** and may be removed no earlier than an explicitly reviewed `0.6.0` with migration notes.
No compatibility baseline is regenerated to make a test pass.

The current services remain the only writers. No dual-write/shadow scheduler is introduced. Future
core writes must fail closed when their durable store cannot confirm them. For a first cutover prefer
an explicit maintenance window: stop old writes, reconcile final changes, drain/inspect runtime work,
then route both old and new APIs through one authoritative core. Before new writes, rollback may switch
back; after new writes, reconciliation and runtime ownership checks are mandatory. Old table deletion,
privilege expansion and deployment remain separate decisions.

## 7. Verification and next gate

The new tests were added first and failed because the modules did not exist. The implementation then
passed 9 contract tests and 7 projection tests. The ID distinction also has a compile-fail doctest.
Use the standard backend gate to run them alongside existing regressions:

```bash
CARGO_BUILD_JOBS=2 ./scripts/quality-gate.sh --backend-only
```

Run tests with a disposable `CYANREX_DATA_DIR`, without a production `DATABASE_URL` or inherited
deployment credentials. Do not use a production database to run the ignored PostgreSQL integrations.

This slice passed the complete default Rust suite (including the compile-fail doctest), common
preflight/format checks with 71 tooling tests, 26 frontend permission/workflow tests, and SDK checks
(13 runtime tests, type checks and 3 package tests). OpenAPI/SDK generation and compatibility stayed
in sync. Optional PostgreSQL integrations, real kernel/LAN acceptance, browser end-to-end tests and
live dependency audits were not run for this slice; no deployment acceptance is claimed.

| Gate | Current delivery / remaining work |
|---|---|
| C0 source baseline | Source storage/permission/API inventory and decisions recorded here. |
| C0 deployment baseline | **Pending**: confirm real instances, durable/fallback sources, restore-tested backups and active resource ownership. |
| C-M1 | Typed contracts and offline role projection delivered; no runtime identity registry or policy switch. |
| C1 | [C1-A](collaboration-identity-store.md) identity mappings, [C1-B](collaboration-access-store.md) membership/deployment policy, and the 0.4.5 [C1-C](collaboration-policy-audit.md) policy commands/audit and [C1-D](collaboration-identity-lifecycle.md) attributed binding/retirement staging are implemented. In 0.4.6, [C1-E](collaboration-auth-source.md) adds a durable account/session source; [C1-F](collaboration-session-commands.md) composes session-authorized binding/policy transactions. Full account lifecycle, general grants and live protected-operation integration remain pending. |
| C-M2 | Pending: fixture-backed offline attempt conversion and full ID/digest/owner/progress/review reconciliation; first synthetic, then approved isolated copies. |

No new product release, commit, tag or deployment is created by this foundation slice.

[auth]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/auth_service.rs
[routes]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/application.rs
[guards]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/routes/auth_session.rs
[learning]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/routes/learning.rs
[progress]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/learning_store/queries.rs
[scripts]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/script_store.rs
[learning-store]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/learning_store.rs
[migrations]: https://github.com/Team-silvortex/cyanrex-lab/tree/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/migrations
[runner]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/runner_driver.rs
