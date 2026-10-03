# ADR-020 Session authorized private human reviews

Status: **C2-H included in source release 0.4.9**. Decision date: **2026-10-03**.

This slice composes a current Session, private human Review history and exact owned Artifact targets
and evidence on one source transaction. It extends the [Review store](review-record-store.md), not
the public teaching workflow. A personal opinion, including `approved`, is not Task acceptance,
permission to review another member's work or proof that an assessment policy ran. Source release
0.4.9 does not switch any live route, authentication source, schema or deployed data.

## Configuration and operations

The Unix-only `SessionReviewWorkspace::new(namespace, scope, artifacts, policy)` accepts trusted
server configuration. Its fields are private and it cannot be deserialized from a request. The
Review namespace, the existing `SessionArtifactWorkspace` and the source/registry namespace must
be distinct, already initialized namespaces in one database; both resources use the same authority
and Workspace. The exact policy name and version come from this configuration, not a command body.
Configuration does not install or execute a policy, create directories or repair storage.

| Source operation | Command input beyond Session and configuration | Confirmed result |
|---|---|---|
| `create_session_review` | Review ID, exact targets, exact evidence and `HumanReviewEdit` | Human Review revision 1 |
| `read_session_review` | Exact `ReviewRef` | Requested historical record, or none for absent, future or other-owned references |
| `revise_session_review` | Expected current `ReviewRef` and `HumanReviewEdit` | Next immutable human revision |

The reviewer is derived from the current account incarnation, audited active Human, active Workspace
and audited active membership. Teaching or deployment grants are neither required nor sufficient
for access to another person's private Review or content. The adapter accepts no caller-selected
reviewer, source, policy, rule result or domain definition. It rejects Rule records and Human records
whose exact policy differs from the configured policy. A policy change requires a new Review identity;
old opinions are not silently upgraded.

Targets contain 1–32 exact Artifact references; optional evidence contains 0–32. Each reference pins
Workspace, Artifact ID, revision ID and digest. Duplicate coordinates within either list and conflicting
digests across lists are rejected. The same exact reference may be both a target and evidence. Every
target and every evidence item must be owned by this Session's Principal and resolve to verified bytes.
Matching someone else's digest does not grant access.

Edits change only verdict and comment plus revision, time and predecessor. Reviewer, targets, evidence
and policy remain fixed. The underlying comment and 64 KiB serialized-record limits still apply.
A newer Artifact version never moves a saved Review reference to latest. Reads return only the record,
not the content bytes; a historical opinion may already have been superseded.

## Transaction and lock boundaries

The source owns the connection, transaction and commit. The shared private-work guard verifies the
source and audited membership before resource work. Review metadata and the current owned head are
locked before Artifact metadata and heads. Targets and evidence are combined, globally sorted by
Artifact/revision ID and deduplicated only when the complete references match. Content is checked
one revision at a time and discarded, keeping this operation independent of Task, Run or teaching code.

Borrowed Review reads lock the current head under READ COMMITTED before checking its latest event
and the requested history. They preserve the store's fixed-subject comparison. Standalone trusted
Review reads retain their separate REPEATABLE READ, READ ONLY snapshot; that unlocked implementation
is not used as authorization inside a source transaction.

Before any Review write, the guard pins the original Artifact namespace name and OID. After writing
Review head, revision and outbox state, the adapter verifies all content again, then rereads the pending
Review. This catches same-transaction trigger side effects that row locks do not exclude. Namespace
revisits check the original pin rather than adopting a same-named replacement. Final checks verify
all visited namespaces, source, identity and membership, then the exact Session using fresh database
time after waits. No record is returned until those checks and the single commit are confirmed.
The final Session check does not promise continuous validity during an arbitrarily delayed COMMIT.

## Failure and authority limits

Missing, other-owned, mismatched or corrupt content blocks creation, reading and amendment. Evidence
is never silently dropped. Review/history/event failures, namespace substitution, stale revisions,
revoked membership or expired Sessions do not produce successful records. Operations keep the existing
ten-second deadline, five-second statement timeout and two-second lock timeout, with no memory fallback.
Transaction-local search paths must be restored on success, failure and cancellation.

This adapter does not write, delete or repair Artifact files or emit Artifact mutation events. A
timed-out blocking file read may continue. Cancellation, timeout and a lost commit acknowledgement
do not prove rollback, and there is no automatic retry, recovery or successful replay receipt.
Checks protect cooperating writers, not coordinated database tampering or same-UID changes after
a file check. Exact-content validation proves neither authorship nor semantic evidence sufficiency.

Trusted direct Review and Artifact writers remain available. There is no provenance marker or cutover
fence, so an accessible historical record is not certified as originally Session-authored. The pinned
policy is an explicit configuration label, not proof of policy installation, evaluation or authority.
`Approved` remains a private human opinion and cannot advance a Task. Rule/Agent issuance, cross-user
review grants, applicable domain policy, Task acceptance, public HTTP/CSRF/UI, migration, retention and
reliable Run/event delivery remain separate work. C2 and C-M2/C-M5 are not complete.

## Verification

Three default tests and eighteen explicit PostgreSQL cases target configuration and bounds,
private ownership without teaching roles,
exact history and policy pins, lifecycle revocation, damaged targets/evidence, suppressed writes,
commit failure, namespace substitution, lock ordering, Session expiry/logout and cancellation.
The explicit `scripts/test-session-review-storage.sh` CI runner checks every selected test name
before using a disposable PostgreSQL database.
Executed results and checks not rerun are recorded in [project status](project-status.md); this
coverage description does not claim that every check has passed. Never run fault fixtures on deployed data.
