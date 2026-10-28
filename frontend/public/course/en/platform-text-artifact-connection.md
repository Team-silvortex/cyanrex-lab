# Retained platform text Artifact publication

Source **0.6.5** includes the Task-bound connector, strict HTTP adapter and default-off workspace/CLI
opt-in below. Their `Unreleased` headings/anchors preserve implementation stages, not pending release
status; dated evidence keeps its original scope. The seven UI assets still have no raw-text save client.

The original unbound capability, included in source version **0.6.4**, extends
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3) with explicit creation of
one private text Artifact over an original retained storage binding. Notes and configuration are
valid content; code, a Task and a Run are not required.

The connector itself adds no transport. A separate `TextArtifactPublishHttpState` and
`build_text_artifact_publish_router` adapt it to standalone HTTP; explicit host composition is below.
Neither changes [Task manifest replacement](platform-task-replace-connection.md), connects
browser text/SDK saving or replaces teaching authentication. Existing pooled Artifact APIs retain their policy.

## Explicit capability

`PlatformTextArtifactConnector::new(storage)` consumes an original `PlatformStorageConnector`
without I/O. `origin()` and `scope()` expose configuration metadata, not reusable authority.
Its only resource command is:

```rust
create_session_text_artifact(
    &self,
    token: &str,
    artifact: ArtifactId,
    revision: ArtifactRevisionId,
    title: String,
    text: String,
) -> Result<ArtifactRevision, SessionArtifactError>
```

The caller supplies new Artifact and revision identities, not an owner, path, digest, parent,
sequence, kind or media type. Current Session authority determines the owner inside the transaction.
A teacher role does not grant another owner's private content permissions.

## Fixed text profile

Publication fixes kind to `cyanrex.task-text` and media type to `text/plain`.
Text is UTF-8 and at most **256 KiB in bytes**, including multibyte characters. Empty text is valid;
control characters other than tab, newline and carriage return are rejected. Titles must be nonblank,
at most 256 UTF-8 bytes and contain no control characters.

Token and text/title validation happen before connecting. This profile is narrower than the generic
Artifact store; it neither relabels arbitrary binary content nor selects an editor or execution policy.

## Standalone HTTP

`TextArtifactPublishHttpState::new(connector)` accepts only the retained connector, whose configured
Origin is trusted. `build_text_artifact_publish_router(state)` exposes only exact
`POST /platform/v1/text-artifacts`, without a query. The strict existing body has exactly five keys:
`schema_version: 1`, `artifact_id`, `revision_id`, `title` and `text`. Unknown or duplicate fields and
positional arrays are rejected. Confirmed creation returns **201** with only `schema_version` and
`artifact`; it does not echo text or issue a Cookie.

Exact Origin, `x-cyanrex-platform-request: 1` and the dedicated `cyanrex_platform_session` Cookie
are mandatory. Route and credential rejection precede body polling. Actual streamed wire bytes are
limited to **2 MiB** with a **two-second body deadline**; decoded text remains limited to **256 KiB**.
Title/text validation finishes before dispatch. The body deadline never wraps the connector command.

Each state defaults to **four response slots**, configurable from **1–16** with `with_capacity`.
Clones share that independent lane. An admitted success or error holds its slot until the final
response-body frame or drop, not merely handler return. This is separate from actual file-job slots.
Failures before dispatch report `not_attempted`; command errors report `unconfirmed`, not rollback
or permission to retry. The adapter adds no command timeout, retry, read route, UI/SDK merge,
listener or teaching login change. The older pooled text router remains separate.

## Explicit host composition

`PlatformWorkspaceHttpState::with_text_publication_capacities` explicitly enables **eight routes and
seven independent response lanes**. It validates all seven capacities before splitting one retained
storage `Arc` into seven capabilities. Original constructors, including `new` and
`with_replace_capacities`, still disable publication: **seven routes and six lanes**.
Without the Task-bound flag below, the [plain](platform-loopback-host.md) and
[native-TLS](platform-loopback-tls-host.md) CLI hosts select this constructor with four slots in every
lane. Their global **16-connection** admission remains; seven four-slot lanes do not promise 28
concurrent requests. Original storage, four actual text file-job slots and private error outcomes
are unchanged. No pooled text read, file/status route or teaching AppState is merged.

## One original transaction

Each call opens one fresh original connection and uses `BEGIN ISOLATION LEVEL READ COMMITTED`.
It does not override a database role's explicit `READ ONLY` policy or fall back to a pool.

1. Verify the original catalog, all five storage markers and retained Artifact root.
2. Resolve the current Session account incarnation and audited active Human membership in the
   configured workspace; retain original Source and target namespace identities.
3. Create one sequence-1, parent-free Artifact head, immutable revision and matching outbox entry.
   The private file is exclusively created, synchronized and read back.
4. Execute `SET CONSTRAINTS ALL IMMEDIATE` before checking deferred effects.
5. Recheck namespace shape, the **complete exact current head**, revision/outbox agreement and actual
   bytes. An earlier revision remaining readable does not confirm this creation if a deferred trigger
   has installed a different valid head.
6. Restore the original Source and recheck reader/root, retained storage, current actor and fresh
   exact Session expiry after all guard waits.
7. Return the pending revision only after acknowledged COMMIT.

Artifact publication does not create a Task or replace its manifest. Linking the resulting exact
revision to a Draft is a separate authorized operation.

## Unreleased: Task-bound publication capability

`PlatformTaskTextArtifactConnector::new(storage)` is a separately selected capability with the
standalone HTTP adapter below and explicitly opted-in workspace/CLI composition; it is not a browser operation.
`create_session_task_text_artifact(token, expected: TaskContentSnapshot, artifact, revision, text)`
accepts a complete expected manual Task/manifest snapshot and new Artifact/revision identities.
The Artifact title derives from that expected Task; the fixed text profile above still applies.
The supplied owner is only an exact consistency pin against the current Session actor, not a selected
account, grant or substitute for authentication. A teacher still cannot publish for another owner.

Token, structural scope/manifest and text-profile checks precede the one original connection.
One `PrivateWorkContext` in the same READ COMMITTED transaction then:

1. Resolves current audited Session authority and locks the exact current owned Task. Scope, owner,
   expected revision, Draft status and the complete Task/manifest baseline must agree before bytes.
2. Calls `recheck_selected_for` after the Task lock wait: rechecks original Source, current actor and
   fresh exact Session expiry, then returns to the same pinned Task target before file dispatch.
3. Creates the new Artifact through the retained writer, flushes deferred constraints and verifies
   its exact new head/revision/outbox and real bytes. It then rereads the unchanged complete Task.
4. Rechecks original root/storage, followed by final fresh Source/actor/Session checks immediately
   before acknowledged COMMIT. No detached Task GET or reusable authorization boolean is used.

This is a **Task metadata guard**, not verification of old payload bytes or a completed Task save.
It does not change the Task revision, manifest or outbox, attach the new reference, publish a whole
draft atomically or restore a save attempt. Linking still requires a separately authorized replacement;
its failure cannot undo this confirmed publication. Unknown outcomes retain files without retry,
adoption or cleanup, as below.
This is not idempotency or a one-publication-per-Task-revision rule: distinct new IDs may publish
against the same unchanged baseline. Duplicate IDs conflict rather than recover an earlier receipt.

Relative to the unbound publication profile, trusted provisioning also needs Task SELECT and the
lock-only `UPDATE(task_id)` column grant. There is no Task business UPDATE/INSERT/DELETE or Task-outbox
INSERT, Artifact-head UPDATE, password/TOTP SELECT, automatic grant or override of explicit READ ONLY.
The three Artifact INSERT grants and existing marker locks remain unchanged. This capability keeps
the original retained root and its own four actual-worker slots; it installs no schema or new storage binding.

The existing strict five-field text POST and seven opt-in UI assets stay unchanged. Default CLI
startup retains eight routes/seven lanes; the separate adapter and explicit opt-in composition below
select this capability. Browser, SDK and teaching authentication remain disconnected. This adds no
extra listener, Cookie issuer, migration or cross-owner permission.

## Unreleased: standalone Task-bound HTTP

`TaskTextArtifactPublishHttpState::new(connector)` accepts only the Task-bound retained connector.
`build_task_text_artifact_publish_router(state)` supplies only exact
`POST /platform/v1/tasks/{task_id}/text-artifacts`, without a query or trailing slash. Other methods
on that canonical path return private **405**, `Allow: POST`. The standalone builder remains available;
workspace/CLI inclusion requires the explicit constructor or flag below. The opt-in Session page,
normal application and SDK do not call it.

The body has exactly five fields: `schema_version: 1`, `expected`, `artifact_id`, `revision_id`
and `text`. `expected` has exactly `task` and `manifest`: the complete existing manual Task snapshot
and text manifest, not a desired new Task. The canonical path ID must match `expected.task.reference.task_id`.
Trusted scope, owner-authority and manifest/Task structural agreement are checked before dispatch;
only the connector's current-Session transaction establishes actual ownership and current state.
The Artifact title derives from that Task; no separate title, owner selection or path is accepted.

All record levels admit maps only, including Task/reference/owner/workspace, input-reference items
and the manifest's nested records. Unknown, duplicate semantic or missing fields and positional
record arrays fail before dispatch. `definition` is required and must be explicit `null`; status
is one of the existing six strings, never a unit-enum object. Existing schema, canonical UUID,
digest and JSON-safe integer rules still apply. Timestamps are time-value pins: equivalent RFC3339
offsets may denote the same instant; no canonical UTC string spelling is promised. A single escaped
known field name is ordinary JSON, but repeated names after unescaping are duplicates and rejected.
These wire-only DTOs do not change the shared stored-model deserializers.

Exact trusted Origin, marker and dedicated Session Cookie are required; Authorization is rejected.
Cheap route/header checks precede admission and body polling. Actual wire collection is bounded to
**2 MiB** and **two seconds**, with decoded text still **256 KiB**. Body/profile validation finishes
before dispatch, and the HTTP deadline never wraps the admitted connector command. Each state has
**four response slots** by default, configurable **1–16**; clones share them through admitted success
or error body completion/drop. They are independent of the connector's actual-worker slots; default
CLI composition is unchanged, while the explicit Task-bound opt-in adds a separate response lane.

Only confirmed publication returns **201** with the existing `schema_version` and `artifact`
metadata, not text, a Task, Location or Set-Cookie. Pre-dispatch errors report `not_attempted`;
connector errors report `unconfirmed`. This adapter separately maps Artifact identifier conflicts
to **409 `conflict`** and Artifact stale revisions to **409 `stale_revision`**, without changing
the older Task error mapper. Missing and foreign-owned Tasks remain the same private 404.
It does not attach content, change the Task, verify old payload bytes or supply retry, cleanup,
whole-draft atomicity, idempotency, recovery or browser saving.

## Unreleased: opt-in Task-bound workspace and CLI

`PlatformWorkspaceHttpState::with_task_text_publication_capacities(storage, login, lifecycle,
index, content, create, replace, publish, task_publish)` adds the Task-bound POST to the original
eight routes. It validates **all eight capacities** (each **1–16**) before splitting one original
storage `Arc` into eight capabilities, without opening or observing a replacement baseline.
This opt-in has **nine routes/eight independent response lanes**. It retains the old unbound
`POST /platform/v1/text-artifacts`; enabling it does not make the entire host Task-bound-only.

Existing constructors and signatures retain their old defaults: `new`/`with_replace_capacities`
have seven routes/six lanes, without either publication; `with_text_publication_capacities` has
eight routes/seven lanes, without Task-bound publication. Cloned states share their original lane
budgets. The standalone Task-bound builder still owns its private fallback/headers; explicit workspace
composition merges only its fallback-free route under the workspace's existing private response policy.

For either the [plain startup](platform-loopback-host.md#explicit-startup) or
[native-TLS startup](platform-loopback-tls-host.md#explicit-private-inputs), add the default-off
`--task-text-publication` after the required mode arguments. Optional suffixes must be in this order:

```text
[--task-text-publication] [--notify-systemd] [--session-ui]
```

The flag is accepted only by `serve-loopback` and `serve-loopback-tls`; diagnostics, repeated flags,
aliases, reordered suffixes and trailing arguments are rejected. Without it both CLI hosts keep their
existing eight-route/seven-lane composition. With it they explicitly select the new constructor with
four slots per lane; it does not enable `--session-ui` or add a body-editing client to its seven assets.
Process/private-input/original-storage checks, literal-loopback binding, native TLS verification,
Cookie policy and signal drain retain their existing boundaries. There are no new environment/JSON
configuration entries, schema installation, grants, migration or teaching/SDK fallback.

The two publication connectors each retain **four actual blocking-worker slots**, independently:
the enabled pair can hold eight actual file jobs, not a new shared four-job limit. Read/replace worker
budgets remain separate too. Eight four-slot response lanes are not a guarantee of 32 concurrent
requests; global network admission is still **16 connections**. Trusted provisioning must retain the
combined role's existing login, Task and Artifact/row-lock profile; this opt-in adds no further grants.
It still neither checks old payload bytes nor changes/attaches a Task, retries, cleans up, recovers
uncertain outcomes or establishes browser TLS/LAN acceptance.

## Original files and worker ownership

The writer inherits the retained original directory descriptor. It checks the visible no-follow
root rather than adopting a newly opened acceptable directory. File creation uses exclusive,
descriptor-relative names derived by the server from typed identities and digests; existing files are not overwritten
or adopted. Private regular-file ownership, single-link status, mode `0400`, length and SHA-256 are
verified with root/inode checks.

Each connector owns **four fail-fast actual blocking-job slots**, independent of other connector
budgets. The real queued/running file job holds its slot through completion, including synchronization
and readback, even after its waiting future is cancelled. This is not a four-request transaction limit.
Existing pooled stores keep their prior worker policy.

Connection verification, business work and close have separate ten-second budgets. Synchronous root
checks can still stall; neither those budgets nor file/SQL checks are a hard process deadline,
an atomic filesystem/database transaction or protection against the same Unix identity/root.

## Provisioning and uncertain outcomes

Trusted provisioning supplies the declared original Source, identity/access, Task and Artifact
namespaces; the connector installs or migrates none of them. Existing narrow Session/registry/storage
marker grants remain; Artifact metadata requires SELECT and its schema-marker row-lock column grant.
Publication additionally needs `INSERT` on
`collaboration_artifacts`, `collaboration_artifact_revisions` and `collaboration_artifact_outbox`.
The inserted head is already transaction-owned, so this create needs no Artifact-head UPDATE grant.
It needs no password/TOTP SELECT, Task writes, broad table UPDATE or automatic grants.
The combined host role retains existing login credential SELECT and Task/Artifact row-lock grants;
only the three Artifact INSERT grants are additional. Provisioning must not revoke old lane privileges.

A blob may exist before the SQL transaction is confirmed. Cancellation, timeout, disconnect or a
commit error leaves the result unconfirmed, not proof of rollback. Preserve already-written files;
do not automatically remove them, retry publication or adopt a same-name blob. A later conflict is
not a receipt for the earlier attempt. Confirmed COMMIT remains confirmed if connection close fails.

## Verification and remaining connection

Use only explicitly selected disposable storage:

```bash
bash scripts/test-platform-text-artifact-connection.sh
bash scripts/test-platform-retained-text-http.sh
bash scripts/test-platform-text-publication-host.sh
bash scripts/test-platform-task-text-artifact-connection.sh
bash scripts/test-platform-task-text-artifact-http.sh
bash scripts/test-platform-task-text-publication-host.sh
```

The runner requires a disposable administrative `CYANREX_TEST_DATABASE_URL`; default Cargo selection
does not run ignored SQL cases. Tests must distinguish pre-dispatch validation from real publication,
verify exact metadata and files, exercise deferred faults/current authority/original bindings and
retain cancellation-owned worker budgets. The second runner selects the retained HTTP target,
not the older pooled text adapter. The third inventories six real SQL/workspace/plain/TLS selectors;
inventory is not a passing result.

The fourth runner explicitly inventories seven ignored Task-bound SQL cases: private publication with
unchanged Task/Source, foreign/missing/scope rejection, stale/tampered/status/revoked rejection,
deferred Task drift with retained uncertain blobs, original root/marker drift, narrow grants/READ ONLY,
and an observed Task-row wait crossing Session expiry with zero new files. Its default constructor
shape test is separate; neither inventory nor a source guard establishes that these cases passed.

The fifth runner selects six exact ignored Task-bound HTTP SQL cases. It exercises the independent
adapter with real retained storage; it does not mount the CLI, start a browser save, verify old
payload bytes or establish deployment acceptance. Its selector inventory is not a passing result.

The sixth runner selects six exact ignored workspace/host cases, including real plain/native-TLS
children with explicit temporary client CA trust. Its inventory is not a passing result or browser
TLS/LAN acceptance. It retains original-storage, current-owner/revision, response-budget, unknown-blob,
default-off compatibility and signal-shutdown checks; publication still does not save a Task.

Browser conflicts and uncertain-outcome handling, generic SDK/OpenAPI integration, cross-user grants
and recovery remain separate work. No Artifact read route, extra listener, Cookie issuer, Task status
transition or Run is added here.
