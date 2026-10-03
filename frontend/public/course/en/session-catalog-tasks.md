# ADR-021 Session authorized catalogue tasks

Status: **C2-I included in source release 0.4.9**. Decision date: **2026-10-03**.

This slice joins the [versioned task catalogue](task-domain-boundary.md) to current-Session private
Task operations. A member can create, read and advance a task using an exact server-admitted definition,
with zero to 32 owned Artifact inputs. It reuses the [Task input transaction](session-task-inputs.md)
without adding teaching-specific task kinds, a required Run, rule execution or Task acceptance.
Source release 0.4.9 leaves existing teaching data, public APIs and live authentication unchanged.

## Trusted definition configuration

The Unix-only `SessionCatalogTaskWorkspace::new(tasks, artifacts, catalog)` takes trusted server
configuration. Task and Artifact handles must have distinct namespaces and the same authority and
Workspace. Commands use the source connection to access those already initialized namespaces in
the same database. No installer, migration, filesystem repair or second transaction is invoked.

The handle copies the immutable `TaskCatalog` definition list into private shared storage. It does
not retain the provider or its evidence type, call `assess`, discover packages or execute domain code
during authorization. It can outlive the catalogue/provider used to configure it. The catalogue's
existing 1–256 definition bound applies; clients cannot deserialize or replace the handle's fields.
The copied entries form this handle's explicit definition allowlist, not a dynamic policy registry.

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

Three default tests and sixteen explicit PostgreSQL cases cover a non-teaching provider,
provider lifetime independence, exact versions, same-pin metadata drift, zero-input metadata checks,
ordered private inputs, strict manual boundaries,
lifecycle revocation, revision races, write/namespace faults, lock waits, expiry/logout and cancellation.
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
