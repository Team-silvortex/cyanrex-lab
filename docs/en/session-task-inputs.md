# ADR-019 Session authorized private task inputs

Status: **C2-G included in source release 0.4.9**. Decision date: **2026-10-03**.

A member can now create, read and advance a private task whose inputs are exact revisions of their
own Artifacts. Current Session authorization, Task state/events and input access checks share one
source-owned transaction. This joins the [private Task](session-task-commands.md) and
[private Artifact](session-artifact-commands.md) adapters without adding a domain definition, Run,
Review approval or public workflow. Source release 0.4.9 leaves live teaching data and routes
are unchanged.

## Configuration and operations

The Unix-only `SessionTaskInputsWorkspace::new(SessionTaskWorkspace, SessionArtifactWorkspace)` takes
trusted server configuration, not request-selected namespaces or paths. Both handles must describe
the same authority/Workspace. Task, Artifact and source/registry namespaces must be distinct and already
initialized in the same database. Commands use the source connection, never a second store transaction,
and do not create, adopt, migrate or repair a namespace.

| Source operation | Inputs beyond Session and configuration | Confirmed result |
|---|---|---|
| `create_session_input_task` | Task ID, title and exact `ArtifactRef` inputs | Private Task snapshot at Draft revision 1 |
| `get_session_input_task` | Task ID | `SessionTaskInputs { task, inputs }`, or none for absent or other-owned tasks |
| `transition_session_input_task` | Task ID, expected Task revision and next status | Updated Task snapshot with unchanged inputs |
| `replace_session_input_task_inputs` ([C2-J](session-task-revisions.md)) | Task ID, expected revision and 1–32 exact references | Draft with replaced ordered inputs |

Only tasks with no catalogue definition and 1–32 inputs are supported. References bind Workspace,
Artifact ID, revision ID and digest; duplicate Artifact/revision coordinates are rejected. Each input
must be owned by the current Session's Principal and resolve to verified bytes in the configured
Artifact store. Knowing a digest or being a teacher/deployment manager does not authorize another
owner's content. The existing C2-E manual methods continue to reject all Artifact inputs and definitions.

The input list and title remain fixed during status changes. A newer Artifact revision does not move
an existing input to `latest`; the task still reads the original revision. There is no sharing,
reassignment or definition upgrade. [C2-J](session-task-revisions.md) now adds explicit Draft input
replacement with old/new verification and expected-revision checks. Empty-input manual tasks use C2-E.

[C2-I](session-catalog-tasks.md) reuses this transaction through a separate catalogue adapter
for 0–32 inputs. It copies server-admitted definition metadata without retaining/executing providers;
reads/transitions require equality with the full saved snapshot. Even zero inputs pin and verify
Artifact namespace/schema metadata before and after writes. These C2-G methods still reject every
catalogue definition; metadata admission does not validate typed Evidence or execute assessment policy.

## One authorization and storage transaction

The adapter derives the owner from an exact current account incarnation, active audited Human,
active Workspace and audited active membership. It retains the source and registry locks throughout
resource work. Task schema/metadata and any existing Task row are checked and locked before Artifact
metadata and heads. Inputs are visited in stable ID order for locking, without changing their stored
order. Borrowed Artifact reads lock heads under READ COMMITTED before inspecting versions/events;
they also verify the exact owner, reference, length, digest and private-file constraints.

For creation or transition, the command validates the inputs, writes Task/outbox state, then validates
every input again. It returns to the Task namespace and rereads the complete pending snapshot before
finishing authorization. Input bytes are checked one at a time and discarded by these mutations;
neither operation writes, deletes or repairs Artifact content or emits an Artifact mutation event.

The shared guard retains the first name/OID pin for every visited namespace. Before leaving a target
it verifies the current selection; revisiting a name cannot replace its original pin. Before commit,
it verifies all visited pins, restores the source, rechecks current identity/membership and finally
checks the exact Session against fresh database time. Source names and OIDs cannot be reused as
resource targets. Transaction-local search paths must be restored on success, failure or cancellation.

Reads use this locking authorization path too. Returned `inputs` follow the task's original reference
order and contain only verified content. At most 32 MiB of input bytes can be returned, plus metadata,
from the existing one-MiB per-revision limit; this is not streaming or an aggregate storage quota.
Neither pending metadata nor bytes leave the adapter until the final checks and confirmed commit.
A read is not an access grant for a later command or a repeatable-read maintenance observation.
The final Session check does not promise continuous validity during an arbitrarily delayed COMMIT.

## Failure and cancellation policy

Missing, other-owned, mismatched or corrupt input content rejects the operation; it is not silently
skipped or replaced with empty bytes. This also blocks a transition to `cancelled`: this slice
deliberately requires readable, authorized inputs for every status change. There is no emergency
cancellation or repair bypass. Cancelling a Task still does not stop a Run or release kernel resources.

The existing state graph, expected-revision conflict checks, terminal-state restrictions and atomic
Task/outbox publication remain. Storage, event, namespace, authorization or commit failures do not
publish a successful Task change. Operations retain the ten-second waiting deadline, five-second
statement timeout and two-second lock timeout, without volatile fallback. A timed-out blocking file
read may continue, but this adapter starts no file write.

Cancellation, timeout and a lost commit response do not prove rollback. There is no command receipt,
successful replay, automatic retry, cleanup or recovery. Database locks protect cooperating storage
paths, not changes by a privileged database operator or the same Unix UID after a file check. Content
validation is not proof of authorship, semantic validity, Review evidence policy or acceptance.

## Verification and remaining work

Three default tests and twenty explicit PostgreSQL cases cover configuration and input bounds,
exact revision/order preservation,
private ownership without teaching roles, stale Task revisions, absent or damaged content, event/commit
failures, namespace substitution, lifecycle revocation, lock waits, expiry and cancellation. The
explicit CI runner is `scripts/test-session-task-inputs.sh`; it must verify each selected test exists
before executing it against a disposable database. Fault fixtures must never target deployed data.
Executed results and checks not rerun are recorded separately in [project status](project-status.md);
this coverage description is not a claim that every check has passed.

Trusted direct Task/Artifact writers remain available, with no cutover fence or new provenance marker.
This adapter checks current ownership and content; it does not certify that historical records were
created through a Session command. [C2-H](session-review-commands.md) separately adds
current-Session private human Review commands with exact owned targets/evidence on one transaction.
Its configured policy label does not prove policy execution, historical Session authorship or Task
acceptance. Typed evidence validation, assessment-policy execution, cross-user review/resource authorization,
legacy-writer fencing, public HTTP/CSRF/UI, migration, retention and reliable Run/event delivery remain separate work.
No live AuthService, API/SDK contract or deployment is switched. The subsequent
[C2-J](session-task-revisions.md) changes the fresh Task storage template to schema 2 and rejects
schema 1 without migration; C2 and C-M2/C-M5 are not complete.
