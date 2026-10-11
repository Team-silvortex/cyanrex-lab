# ADR-021 Session authorized catalogue tasks

Source release **0.5.5** (**2026-10-11**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

Status: **C2-I included in source release 0.4.9**. Decision date: **2026-10-03**.

This slice joins the [versioned task catalogue](task-domain-boundary.md) to current-Session private
Task operations. A member can create, read and advance a task using an exact server-admitted definition,
with zero to 32 owned Artifact inputs. It reuses the [Task input transaction](session-task-inputs.md)
without adding teaching-specific task kinds, a required Run, rule execution or Task acceptance.
Source release 0.4.9 leaves existing teaching data, public APIs and live authentication unchanged.

The **2026-10-08 Unreleased follow-up** on source version 0.5.4 adds provider-free metadata
configuration. It changes no authorization transaction, storage schema, route or public API.

## Trusted definition configuration

The Unix-only `SessionCatalogTaskWorkspace::from_definitions(tasks, artifacts, definitions)` takes a
trusted `TaskDefinitionCatalog`. The compatible `new(tasks, artifacts, catalog)` delegates through
`TaskCatalog<P>::definition_catalog()`. Task and Artifact handles must have distinct namespaces and
the same authority and Workspace. Commands use the source connection to access those already initialized namespaces in
the same database. No installer, migration, filesystem repair or second transaction is invoked.

The handle clones the non-generic metadata catalogue, sharing its frozen `Arc` snapshot. No provider
or evidence type need be constructed or retained; authorization calls no `assess`, package discovery
or domain code. It can outlive the original handle or an optional provider. Single-package registration
and name/display limits remain explicit; clients cannot deserialize or replace the
handle's fields. These entries are an explicit definition allowlist, not a dynamic policy registry.

The **2026-10-11 Unreleased follow-up**, on source 0.5.4, also accepts a metadata handle produced by
`TaskDefinitionCatalog::compose`. Trusted configuration can combine independent packages, exact
package/task versions and disjoint same-package registrations before constructing this workspace.
Both input handles and flattened definitions are limited to 1–256; nested composition has the same
global definition limit. Complete-reference overlap is rejected, even for identical metadata, and
the saved order is retained. No new workspace type, provider, registry or authorization branch is
needed; the existing commands below keep their exact-definition checks and schema 2 transaction.

The separate **2026-10-11 Unreleased assessment-binding follow-up**, on source 0.5.4, admits
definitions with both schema/policy pins or explicit absence of both. The same frozen allowlist can
contain both forms. An unbound entry still creates a catalogue Task with `Some(TaskDefinition)`,
not a manual Task, and no assessment or Review is invented. The complete saved definition includes
this paired choice: adding or removing bindings under the same exact reference returns
`DefinitionMismatch`, while missing entries retain `UnknownDefinition`. Input checks, current
ownership and authorization stay identical. Existing bound records need no migration; new
double-null definitions require the new reader, not an old binary or an implicit downgrade path.
See [assessment-binding compatibility](task-domain-boundary.md).

A command chooses only an exact package name/version and definition name/version from this allowlist.
It cannot supply arbitrary definition metadata, evidence schema or policy. Creation copies the whole
admitted `TaskDefinition` into the stored Task. Reads and transitions compare that complete snapshot
against the configured entry, including schema version, references, evidence-schema and assessment-policy
pins, title and summary. A missing exact entry returns `UnknownDefinition`; different metadata under
the same reference returns `DefinitionMismatch`. Neither case falls back to latest or silently rewrites
the Task. Non-catalogue tasks are rejected by this adapter.

Even presentation changes under an existing definition pin require an intentional new version if
old tasks must remain usable. Retain old entries in the configured catalogue to keep operating on
old tasks; multiple definition versions can coexist. Creating a new handle does not revoke an existing
handle or persist a registry change. Package authenticity, durable catalogue lifecycle and coordinated
configuration replacement remain outside this staging boundary.

## Operations and input boundaries

| Source operation | Inputs beyond Session and configuration | Confirmed result |
|---|---|---|
| `create_session_catalog_task` | Task ID, exact definition reference, title, 0–32 Artifact references | Draft Task revision 1 with the copied definition |
| `get_session_catalog_task` | Task ID | `SessionTaskInputs { task, inputs }`, or none for absent or other-owned Tasks |
| `transition_session_catalog_task` | Task ID, expected revision and next existing status | Task with unchanged title, definition and inputs |
| `replace_session_catalog_task_inputs` ([C2-J](session-task-revisions.md)) | Task ID, expected revision and 0–32 exact references | Draft with replaced ordered inputs and unchanged definition |

Current account incarnation, audited active Human membership and active Workspace determine the
owner. A teacher/deployment grant does not widen access to another member's private tasks or inputs.
Existing manual methods remain strict: C2-E accepts only definition-free, reference-free tasks; C2-G
accepts only definition-free tasks with 1–32 inputs. Neither becomes a catalogue bypass.

Exact inputs retain Workspace, Artifact ID, revision ID and digest. Duplicate coordinates, oversized
lists, missing content, other ownership or mismatched/corrupt bytes fail closed. Reads preserve the
saved order and old Artifact versions, returning at most 32 MiB of bytes plus metadata. Input checks
do not convert bytes to a provider's typed Evidence or validate its semantic evidence schema. An
Artifact kind or MIME label alone cannot establish that typed contract.

Zero-input catalogue tasks return an empty `inputs` list, without inventing a file or Run. They still
pin the Artifact namespace and verify/lock its schema metadata and scope, both before and after Task
writes. Empty input is not permission to accept missing, replaced or wrong-scope storage. No content
file is read for that case; construction still requires an existing private Artifact root.

## Separate catalogue content composition

The **2026-10-08 follow-up, source 0.5.4, Unreleased** adds
[`SessionCatalogTaskContentWorkspace`](session-task-content.md) for catalogue Tasks with titles,
filename/language labels and ordered text references. It takes the same metadata-only catalogue, but
uses the existing **schema 3 content store**, not this schema 2 generic-input adapter. Its four
`*_session_catalog_content_task` commands share the existing content authorization executor: each
text item is at most 256 KiB and must satisfy the UTF-8/control-character contract, with old/new bytes
checked around a mutation. It does not convert these texts into domain Evidence or execute pinned
schema/policy rules.

This page's commands and generic-input limits remain unchanged. The new content handle cannot be
passed to manual content, HTTP or draft-publication entry points. Catalogue content creation rejects
unknown references as `UnknownDefinition`; an owner-visible saved definition that is missing or no
longer exactly configured is `InvalidRecord`, the content store's record-consistency error. This is
separate from the `UnknownDefinition`/`DefinitionMismatch` contract of the schema 2 commands above.
No format upgrade, public route, browser saving or automatic conversion is added.

## Shared authorization transaction

Catalogue and manual-input tasks use the same source-owned transaction executor, selected through a
private policy branch. Source and registry locks precede Task metadata/head locks, followed by Artifact
metadata and stable Artifact-head order. Namespace identity is pinned before writes. Definition admission
and exact content are checked while the Task is locked; mutations recheck content and the resulting
Task before the shared guard rechecks every namespace, identity, membership and exact Session using
fresh database time. Only a confirmed commit publishes records or retained bytes.

The existing revision-fenced state graph is unchanged. There is no accepted/completed state and no
automatic transition based on a rule result or private `approved` Review. Unknown definitions or
damaged inputs also block cancellation through this adapter; no repair or emergency bypass is added.
The ten-second command deadline, five-second statement timeout and two-second lock timeout remain.
Transaction-local search paths must be restored on success, failure or cancellation.

No Artifact file or event is written or deleted. File checks do not prevent same-UID modification after
verification; the final Session check does not promise validity during an arbitrarily delayed COMMIT.
Timeout/cancellation/lost acknowledgement does not prove rollback. There is no automatic retry, success
replay receipt, content adoption or recovery. Trusted direct Task/Artifact writers remain available:
matching metadata is not proof that a stored Task was originally created through a Session command.

## Verification and remaining work

The original C2-I inventory has three default tests and sixteen explicit PostgreSQL cases for a non-teaching provider,
provider lifetime independence, exact versions, same-pin metadata drift, zero-input metadata checks,
ordered private inputs, strict manual boundaries,
lifecycle revocation, revision races, write/namespace faults, lock waits, expiry/logout and cancellation.
The metadata-only follow-up adds two constructor cases, bringing that explicit SQL inventory to
eighteen; these check exact definitions/private inputs and refusal of drift or unknown pins without
an evaluator. Inventory is not a claim that an earlier run covered the new cases.
The 2026-10-11 multi-package follow-up adds `session_catalog_task/composition.rs`, bringing the current
exact runner inventory to nineteen. It checks composed admission, retained definitions, private inputs
and refusal of missing or changed configuration through the existing command family.
The separate assessment-binding case in `session_catalog_task/assessment_binding.rs` brings the
current exact inventory to twenty, covering a mixed allowlist, explicit unbound Tasks, unchanged
private inputs and rejected binding drift in both directions. These are source counts, not new
execution evidence.
The explicit `scripts/test-session-catalog-tasks.sh` CI runner checks each selected case exists
before execution on disposable PostgreSQL.
Actual results and checks not rerun are recorded in [project status](project-status.md), separately from
this coverage description. Fault fixtures must never target deployed data.

This is definition admission and metadata consistency, not domain evidence validation or assessment-policy
execution. Authenticated rule Review issuance, cross-user resource/reviewer grants, historical writer
provenance, acceptance policy, public HTTP/CSRF/UI, migration and live cutover remain separate work.
The original C2-I slice changes no schema. Subsequent [C2-J](session-task-revisions.md) requires fresh
Task storage schema 2 and rejects schema 1 without migration. Frozen API/SDK contracts remain unchanged;
C2 and the multi-person C-M5 workflow remain incomplete.
