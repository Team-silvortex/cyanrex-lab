# Session authorized task content

Source release **0.5.5** (**2026-10-11**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

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
and exact revision pins. Filename and language are inert labels. The original manual workspace does
not admit catalogue definitions. Neither workspace executes content, accepts binary attachments or
grants shared access; a pinned assessment policy is metadata, not an executed acceptance rule.

## Catalogue content commands included in 0.5.5

The **2026-10-08 follow-up, source 0.5.4, Unreleased** adds the separate typed
`SessionCatalogTaskContentWorkspace::from_definitions(namespace, scope, artifacts, &catalog)`.
It first applies the same trusted namespace/workspace checks, then retains an immutable
`TaskDefinitionCatalog` snapshot. It needs no `TaskProvider` or evidence evaluator.

The **2026-10-11 Unreleased multi-package follow-up**, on source 0.5.4, lets that constructor accept
an explicitly composed metadata catalogue without changing the workspace or commands. Independent
packages, exact versions and disjoint same-package entries retain their full references; duplicate
references fail before workspace configuration. Composition keeps 1–256 input handles and at most
256 flattened definitions, including nested inputs. The four commands still require the complete
configured definition and the same current authorization/text checks, not a provider or latest-version choice.

The separate **2026-10-11 Unreleased assessment-binding follow-up**, on source 0.5.4, permits a
catalogue definition with both assessment pins or with both explicitly absent. A frozen catalogue
may contain both; an unbound Task still has `Some(TaskDefinition)` and does not enter manual mode.
Both nullable JSON fields remain required. The same complete-definition checks reject binding
changes in either direction under an existing reference as `InvalidRecord`. Commands still verify
the exact old/new texts, Task revision and current Session; no assessment or Review is dispatched.
Schema 3 and legacy bound encoding remain unchanged. New double-null records require the new
reader, without a migration or an old-binary downgrade guarantee; see [the contract](task-domain-boundary.md).

| Command | Catalogue-specific contract |
|---|---|
| `create_session_catalog_content_task` | Resolve the exact `TaskDefinitionRef` from the configured catalogue and create a private Draft with that full definition and the supplied text manifest |
| `get_session_catalog_content_task` | Read only a catalogue Task whose complete stored definition still matches the configured snapshot |
| `replace_session_catalog_content_task` | Atomically edit the Draft's title, labels and ordered text pins, preserving its definition |
| `transition_session_catalog_content_task` | Apply the same lifecycle/revision checks, preserving both definition and manifest |

An unknown creation reference is `UnknownDefinition` before database work. For an owner-visible stored
Task, a missing definition, an unknown catalogue entry or any complete-definition mismatch is
`InvalidRecord`. The manual workspace rejects catalogue records, and the catalogue workspace rejects
manual records. Neither mode reinterprets a record or promotes a manual Task into a catalogue Task.
The manifest title is independent of the definition's display title. The explicit paired
evidence-schema/assessment-policy choice is preserved, but this composition executes neither contract.

Both workspace types use the same source-owned content executor and the authorization/byte checks
below. They use the existing schema 3 with no migration. The new wrapper cannot be passed to the old
manual commands, C2-N HTTP adapter or C2-P–W draft-publication workflows; those entry points remain
manual-only. The metadata/workspace composition itself does not mount HTTP or connect browser saving.

The separate **2026-10-11 Unreleased [C2-Y HTTP adapter](catalog-task-content-http.md)**, on source
0.5.4, explicitly accepts this catalogue workspace through `CatalogTaskContentHttpState`. Its
`/platform/v1/catalog-tasks` commands select only an exact configured definition reference; the
complete definition, old/new text and current-Session checks stay in these same source-owned
transactions. Both bound and unbound definitions remain catalogue Tasks without assessment.
The old manual HTTP and C2-P–W paths remain unchanged. C2-Y has no normal-app/OpenAPI/SDK/browser
connection, Session issuer, record migration or automatic router composition.

The independent **2026-10-11 Unreleased [C2-Z catalogue publication attempt](session-catalog-task-draft-publication.md)**,
on source 0.5.4 outside the published tag, separately consumes a C2-O plan and exact configured reference.
Preparation freezes the complete definition without I/O; explicit advances publish each Artifact,
then call `create_session_catalog_content_task`. That final transaction still derives the current
owner and rereads actual blobs. Confirmation requires the expected manifest and complete frozen
`Some` definition, preserving bound/unbound catalogue identity without assessment. The old manual
C2-P–W entry points remain unchanged; the new attempt has no checkpoint/journal/observation/recovery,
HTTP, issuer or browser connection, and no atomicity across its independent writes.

The separate **2026-10-11 Unreleased [C2-AA catalogue observation](session-catalog-task-draft-observation.md)**,
on source 0.5.4 outside the published tag, uses `get_session_catalog_content_task` for one exact
unconfirmed C2-Z Task target. Existing current-definition/current-byte validation runs before comparison
with the attempt's full frozen definition and planned ordered text. Typed corruption cannot become a
valid difference or invisibility; a missing Task still does not inspect the Artifact namespace. This
read-only branch does not adopt confirmation, unlock the attempt or grant retry or public access.

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
The multi-package `session_task_content/catalog_composition.rs` case extends the exact runner inventory
to 32, covering composed definitions through schema 3 without weakening manual-mode separation,
private ownership, complete-definition checks or text/revision verification.
The separate `session_task_content/catalog_assessment_binding.rs` case brings the current inventory
to 33, covering mixed bound/unbound definitions, explicit null persistence and rejected binding drift
through the unchanged content commands. Inventory counts do not establish execution.

[C2-N](task-content-http.md) now defines a separately constructed HTTP admission layer over the manual
commands. It is not mounted in the normal application and does not issue sessions or publish content.
The remaining browser path needs explicit secure login/installation and a public boundary around the
internal publication/mapping steps, then save/read/conflict and uncertain-outcome handling. Local editor
IDs and downloaded JSON are not server identities or confirmed saves. Migration, sharing, Review
authority and execution remain separate work.
