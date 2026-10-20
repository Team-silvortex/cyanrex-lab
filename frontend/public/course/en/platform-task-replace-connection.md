# Retained platform Draft manifest replacement

Included in source **0.6.3**, the independent host can replace the current owner's manual Draft
manifest under [host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). After
[empty creation](platform-task-create-connection.md), a client can change its title, labels, order or
references to existing owned text revisions, then [reopen it](platform-task-content-connection.md).
Code is optional; an empty manifest remains valid. This command publishes no new Artifact bytes and
does not connect browser saving, a Run, catalogue definitions or the teaching application's authentication.

## Whole manifest and optimistic revision

Only exact `PUT /platform/v1/tasks/{task_id}/content` is accepted, with a canonical nonzero Task UUID
and no query or trailing slash. The existing strict replacement DTO is reused:

```json
{"schema_version":1,"expected_revision":1,"manifest":{"schema_version":1,"title":"Discussion notes","payload":[]}}
```

The body must be an object with exactly these fields. Duplicate/unknown fields, positional arrays,
wrong types, missing keys and invalid nested manifest fields fail before dispatch. A text binding
contains an exact Artifact revision/hash plus display filename and language; neither filename nor
language grants filesystem access or execution. Requests cannot select an owner, namespace, root,
workspace, catalogue definition or status. Artifact coordinates must match the configured workspace
and are independently authorized inside the transaction, not trusted because they parsed.

Replacement requires the exact current revision, Draft status and a changed manifest. A stale revision
is a conflict, non-Draft work rejects the transition, and an identical manifest rejects a no-op. Missing
and foreign-owned Tasks return the same private absence. Unsupported catalogue or corrupt owned
records fail closed. A teacher role does not grant another owner's private write access.

Success returns **200** with `schema_version`, the updated `task` and `manifest`, without `contents`,
Location or Set-Cookie. It advances the revision once, updates title/input references/time, preserves
Task identity, owner, creation time and Draft status, and appends exactly one matching outbox event.
Concurrent requests with the same expected revision cannot both succeed. The response confirms COMMIT,
not client receipt or an idempotency/retry right. The [private index](platform-task-index-connection.md)
and content GET subsequently observe the saved version under their own current authorization.

## Original storage and complete content validation

`PlatformTaskReplaceConnector::new(storage)` consumes one original `PlatformStorageConnector` with
no I/O. It has only replacement, Origin and scope accessors, no pool or caller-supplied owner/path.
Token and manifest-scope validation precede connecting. Each command uses one fresh original connection
and READ COMMITTED transaction, without overriding explicit READ ONLY policy. The root reader duplicates
the retained original descriptor rather than opening a new acceptable baseline.

The command checks original catalog/five markers and resolves the exact current Session account
generation and audited active Human membership. It locks and strictly reads the owned Task first.
After revision/status/no-op checks, the complete old/new payload union is sorted globally by Artifact
and revision identifier. Duplicate coordinates are retained, so a valid old digest cannot hide a
conflicting new pin. Each reference must have the owned head, exact revision and matching outbox,
and its private regular file must satisfy original-root, inode, length, SHA-256 and text-profile rules.
Removing the last item does not bypass validation of that item's saved content.

Task replacement shares the existing pool-free manual/catalogue storage policy and unchanged SQL.
The retained command alone owns `SET CONSTRAINTS ALL IMMEDIATE` after its head/outbox write. It then
revalidates the Artifact namespace/table shape and **all old and new bytes**, including removed items,
before rereading the exact expected Task/event/manifest pair. It restores the original Source, checks
the reader/root and original storage again, and finally repeats current account/membership and fresh
exact Session expiry immediately before COMMIT. No pending snapshot escapes earlier. Existing pooled
APIs retain their previous permission, worker and deferred-work policy.

Each manifest has at most 32 text bindings, each at most 256 KiB. The union can contain 64 checks;
replacement discards each byte buffer after checking it rather than aggregating a content response.
Each retained replacement connector has four fail-fast blocking-work slots. The actual blocking job
holds its slot through completion even if its waiter is cancelled; this budget is independent of the
readonly connector's workers. Connection verification, business work and close have separate ten-second
budgets. Synchronous root checks can still stall the thread. These checks are not same-UID/root
isolation, a hard process deadline or an atomic filesystem/database transaction.

Confirmed COMMIT remains confirmed if closing the connection fails. Cancellation, timeout, disconnect
and dispatched service errors remain **unconfirmed**, not proof of rollback or authority for automatic
retry, adoption, file deletion or compensation. A later stale revision does not resolve an earlier
unknown outcome. Tests exercise cancellation and dropped delivery, not a lost COMMIT acknowledgement.

## HTTP and six independent lanes

`TaskContentReplaceHttpState::new(connector)` or `with_capacity(connector, capacity)` selects only
this capability. `build_task_content_replace_router` mounts only the content PUT path. It uses the
exact selected Origin, `x-cyanrex-platform-request: 1` and dedicated Session Cookie, rejects
Authorization, and collects at most 64 KiB within two seconds **before** dispatch. No HTTP timeout
wraps the dispatched mutation. Other methods on this path return private 405 with `Allow: PUT`.
Errors before dispatch report `not_attempted`; dispatched errors retain `unconfirmed`, including stale
revision and invalid input. Responses use no-store/no-cache/nosniff, without CORS, redirects or tokens.

The explicit plain/native-TLS loopback hosts now have seven concrete routes and six independent
response-lifetime lanes: login, lifecycle, index, content read, empty create and manifest replace.
`with_replace_capacities(storage, login, lifecycle, index, content, create, replace)` validates all six
capacities before splitting one original storage Arc into six capabilities. Each lane is 1–16,
default four; clones share permits through the final body frame/drop. Older three/four/five-capacity
constructors retain their signatures and default additional lanes to four. Session-only, readonly
and empty-create-only builders stay unchanged; the full pooled write router is not merged.
SDK/OpenAPI, frontend transport, LAN/DNS listening, Task status changes and file distribution remain separate.

## Provisioning and verification

Trusted provisioning supplies original Source 2, identity/access 2, Task 3 and Artifact 1 schemas and
the existing authorization/Artifact-read row-lock profile. Replacement additionally needs Task
`UPDATE(revision,snapshot,manifest)` and Task outbox INSERT, but no Task INSERT or Artifact writes.
Even empty-to-empty edits use the full Artifact read profile, not empty creation's metadata-only
permission profile. No password/TOTP SELECT, installer, migration, automatic grant or role escalation
is added. Reference publication remains a separate trusted operation, not a fallback performed by PUT.

```bash
env CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_content_replace
bash scripts/test-platform-task-replace-connection.sh
```

The runner requires explicit disposable `CYANREX_TEST_DATABASE_URL` administration. Its exact SQL/
HTTP/host cases cover empty edits, exact old/new revisions and ordering, current ownership, corruption,
deferred faults, original bindings, optimistic concurrency, observed wait expiry/cancellation, narrow
privileges and real loopback save/list/reopen. File oracles compare names, device/inode, modes, links
and bytes; they do not claim to undo externally introduced corruption. New text/binary publication,
browser conflict/recovery UI, catalogue work, status transitions, execution and LAN acceptance are not added.
