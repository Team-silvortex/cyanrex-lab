# Retained platform text Artifact publication

Included in source version **0.6.4**, this capability extends
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
The [plain](platform-loopback-host.md) and [native-TLS](platform-loopback-tls-host.md) CLI hosts select the new constructor with four slots in every
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
```

The runner requires a disposable administrative `CYANREX_TEST_DATABASE_URL`; default Cargo selection
does not run ignored SQL cases. Tests must distinguish pre-dispatch validation from real publication,
verify exact metadata and files, exercise deferred faults/current authority/original bindings and
retain cancellation-owned worker budgets. The second runner selects the retained HTTP target,
not the older pooled text adapter. The third inventories six real SQL/workspace/plain/TLS selectors;
inventory is not a passing result.

Browser conflicts and uncertain-outcome handling, generic SDK/OpenAPI integration, cross-user grants
and recovery remain separate work. No Artifact read route, extra listener, Cookie issuer, Task status
transition or Run is added here.
