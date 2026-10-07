# Session authorized task content

Status: **C2-M internal preparation included in 0.5.0**. Decision date: **2026-10-03**.
This adapter joins current Session authorization, [schema 3 content storage](task-content-store.md)
and exact Artifact bytes in one source-owned transaction. It does **not** expose an HTTP command,
connect the browser editor, or replace the live authentication source.

## Command contract

`SessionTaskContentWorkspace` selects a trusted, dedicated Task namespace and an Artifact workspace
in the same authority/workspace. These namespaces must be distinct and non-system. It is configuration,
not deserializable client routing, and does not install or adopt a database.

| Command | Result and constraints |
|---|---|
| `create_session_content_task` | Create a private manual Draft from a checked manifest, with a caller-chosen Task ID and revision 1 |
| `get_session_content_task` | Return the Task/manifest pair and verified bytes in manifest order, or no visible Task |
| `replace_session_content_task` | Replace title, labels and ordered pins together, only in Draft at the expected revision |
| `transition_session_content_task` | Apply the existing lifecycle graph at the expected revision and preserve the manifest |

Every command derives the owner from the current account incarnation and audited active membership.
Being a teacher or workspace administrator is not permission to read another member's private Task.
Creation and editing accept zero to 32 optional text items. An identical manifest is a rejected no-op;
a metadata-only edit is a real edit that increments the same Task revision once.

The [manifest contract](task-content-manifest.md) remains authoritative for title, filename, language
and exact revision pins. Filename and language are inert labels. The adapter neither executes content
nor admits catalogue definitions, binary attachments, shared access or Task acceptance policy.

## Authorization and lock order

The command borrows storage operations into one transaction owned by the durable authentication source.
Source identity and registry locks precede Task metadata and the Task head. Before a Task write, the
guard pins the Artifact namespace and checks every relevant exact reference in a single global
Artifact/revision order. Reads restore manifest order after checking content.

Replacement checks the union of old and new items, including removed items. Equal coordinates with
different digests remain separate checks; a valid old digest cannot mask a forged new one. Changing
only a filename or removing every item cannot bypass checking the old content. Even an empty manifest
requires the selected Artifact storage namespace to be valid.

Each item must match its exact reference and owner, declared byte length and SHA-256 digest. It must
contain valid UTF-8, at most 256 KiB, without control characters except tab, line feed and carriage
return. Mutations validate and release one blob at a time; they do not return payload bytes. Reads
retain only the bounded, validated contents they return. Generic Artifact publication remains a
separate operation and may store bytes that are not admissible as Task text.

After changing the Task and its outbox record, the adapter checks the old/new contents again and
rereads the complete expected Task/manifest pair. The shared guard then rechecks all original namespace
identities, the account incarnation, audited membership and exact Session using fresh database time.
No result is returned before confirmed commit. An error in those checks rolls back pending SQL writes.

## Storage and outcome limits

Schema 3 remains separate from the existing schema 2 manual/input/catalogue adapters. Each format
requires explicit installation in its own fresh namespace; neither version adopts the other. The
trusted `TaskContentStore` is still not an authorization API. Its crate-internal borrowed-transaction
helpers exist for composition and do not expose its private version-specific store handle.

The ten-second command deadline and existing statement/lock timeouts apply. Cancellation, timeout or
lost acknowledgement near commit does not prove rollback. There is no command receipt, automatic retry
or outcome reconciliation. Separately published Artifact revisions are not deleted on a failed edit.
Byte verification does not prevent same-UID modification after a check; final authorization does not
guarantee Session validity during an arbitrarily delayed commit. Namespace and record checks are not
a defense against a privileged database operator or a whole-history audit.

## Verification and next boundary

The separate 0.5.3 [C2-P workflow](session-task-draft-publication.md) calls this create command
only after its explicit Artifact publication steps. It retains confirmed progress in memory, not in
this transaction or schema. C2-M still derives the current owner and rereads actual blobs; a preceding
publication or C2-O value check is not reusable authorization, and no cross-step atomicity is added.

Default API/shape tests and explicit disposable PostgreSQL cases cover this internal boundary.
The exact selection runner is `scripts/test-session-task-content.sh`; dated execution counts and
limitations are recorded in [project status](project-status.md). CI selection is not proof of a remote
CI run or installed-system acceptance.

[C2-N](task-content-http.md) now defines a separately constructed HTTP admission layer over these
commands. It is not mounted in the normal application and does not issue sessions or publish content.
The remaining browser path needs explicit secure login/installation and a public boundary around the
internal publication/mapping steps, then save/read/conflict and uncertain-outcome handling. Local editor
IDs and downloaded JSON are not server identities or confirmed saves. Migration, catalogue content,
sharing, Review authority and execution remain separate work.
