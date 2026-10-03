# ADR-016 Revision bound review records

Status: **C2-D included in source release 0.4.8**. Decision date: **2026-10-03**.

Review is now a separate durable judgment over exact Artifact revisions, without requiring a Task,
Run or teaching role. Human comments retain their revision history; rule results cannot become human
approvals through an edit. This is trusted-adapter PostgreSQL staging, not authenticated review,
verified evidence or task acceptance. This staging slice is included in source release 0.4.8.

## Contracts and source separation

`ReviewRef` identifies authority, Workspace, Review ID and a positive JSON-safe revision. `ReviewRecord`
adds reviewer, exact targets, evidence references, judgment, immediate `supersedes` reference and creation
time. Both source variants are strict, tagged structures; unknown fields and sources are rejected.

| Source | Recorded judgment | Amendment behavior |
|---|---|---|
| `human` | Pinned policy, `approved` / `changes_requested` / `commented`, plain comment | New revision of the same Review |
| `rule` | Exact Task definition/package, policy, evidence schema, `passed` / `not_passed`, feedback | Immutable; a rerun requires a new Review ID |

There is no Agent source until an Agent/session provenance contract exists. A human `approved` value
is a recorded opinion, not proof of reviewer authority or satisfaction of a Task's acceptance policy.
A rule pass is never represented by that human verdict. Review does not modify Task state, install
a policy, execute a rule, publish content or require a fake Run.

`ReviewDraft::human` and `ReviewDraft::rule` have private fields and no deserializer. Human edits use
`HumanReviewEdit`. These constructors validate shape and bounds, not trust: an adapter must obtain the
reviewer/source/policy from authoritative server context, never directly trust browser declarations.
The rule constructor accepts a `TaskAssessment` from trusted code; it does not prove that the declared
rule ran, that its policy is installed, or that the supplied targets were the evaluator's input.

Targets are nonempty and bounded to 32; supporting evidence permits 0–32 references. Each reference
pins Workspace, artifact ID, revision ID and digest. Duplicate coordinates within either list are
rejected, even with a changed digest. An exact target may also be evidence, but conflicting digests
across the lists are rejected. All references must belong to the store's selected Workspace.

Human comments allow up to 8,192 UTF-8 bytes, with tabs/newlines but no other control characters.
`commented` and `changes_requested` require nonblank text; approval may have an empty comment.
Rule feedback keeps catalogue limits: at most 64 messages, 2,048 bytes each and 16 KiB in total.
The definition, schema and policy names must belong to the recorded package namespace. The entire
serialized revision or event is bounded to 64 KiB, independently of these field limits.

## Store operations and history

`ReviewStore::new(pool, scope)` performs no I/O or environment lookup. Its explicit installer accepts
only an empty dedicated namespace; operations neither install nor repair missing tables.

| Operation | Result |
|---|---|
| `create(id, reviewer, draft)` | Revision 1 with no predecessor; a duplicate ID is a conflict |
| `revise(expected, reviewer, edit)` | Human-only next revision after locking and checking the exact current Review reference |
| `read(reference, reviewer)` | Exact historical revision, filtered by reviewer; never a floating current-approval view |

An amendment changes only human verdict/comment and derived revision/time/predecessor. Targets,
evidence, policy and reviewer stay fixed. A new target, policy or rule evaluation requires a separate
Review. An old human approval remains about its original target even after a newer Artifact revision
is published. Reading it does not establish that it is the latest opinion, or that it approves current
content. There is no listing/current-approval projection, deletion, reassignment or acceptance command.

The store does **not** resolve Artifact references, hash content, authorize reading the referenced
resources, verify Principal kind/existence, or compose C1 membership/retirement/revocation checks.
Reviewer filtering is not authentication; knowing a UUID is not a supported credential. The store
must not be exposed as an untrusted public API. There is no route, live state, startup installer,
CLI entry, legacy-review migration or current-authentication change.

## Transactions and failure behavior

Migration template `0013_collaboration_reviews.sql` creates schema metadata, a Review head table,
immutable revision rows and an outbox. Each create/amendment writes its head, revision and event in
one transaction. Events use generated IDs, reviewer attribution and the full resulting record, with
types `cyanrex.review.recorded` and `cyanrex.review.amended`. No previous revision/event is rewritten
by the service. Row counts, post-write scope and readback must agree before confirmed commit succeeds.

Writers lock schema metadata then the Review head. Related reads occur after that lock so a waiting
writer sees the concurrent winner under read-committed isolation. Readers use one read-only,
repeatable-read snapshot. A head must match both the newest stored revision and newest event; a
rewound head cannot hide a later judgment. Each requested revision must match its event, reference
and reviewer. Historical reads also compare their fixed subject/policy with the head. This is bounded
head/requested-revision validation, not full-history reconciliation or protection against coordinated
database tampering. Historical gaps outside the requested revision are not scanned.

Installation rejects public/system/ambiguous paths and existing relations, functions or types. Runtime
checks require four permanent ordinary tables without RLS, partitioning, inheritance or temporary
shadowing; relation locks pin the checked tables. Metadata pins schema version 1 and the scope. These
checks are not a complete schema fingerprint. Snapshot fetches remain bounded even if a database
constraint was removed. Overall operations allow 10 seconds, statements 5 seconds and lock waits
2 seconds. Failures return sanitized errors, not raw SQL, paths, credentials or volatile success.

Cancellation, timeout or a lost commit acknowledgment does not establish rollback. Duplicate writes
are not successful receipt replay; there is no automatic retry, repair, dispatcher, global event order,
consumer cursor or exactly-once delivery. This outbox does not enter the legacy telemetry bus.

## Verification and next boundary

Four default tests cover draft bounds, source separation, unavailable storage and absence of live or
teaching coupling. Seventeen explicit PostgreSQL cases cover preserved history, scopes/reviewers,
concurrent edits, rule immutability, installer refusal, suppressed/deferred writes, cancellation,
consistent reads, post-write corruption, RLS/temp shadows, missing events, pointer rewind, bounded
decoding, lock timeout, overflow and historical subject mismatch. The cross-store case uses verified
Artifact bytes with a text-only Task provider, saves rule and human judgments, publishes new content,
then verifies original targets/bytes and unchanged Task state. It supplies trusted attribution and
does not test an authenticated user workflow or claim automatic evidence verification by ReviewStore.

`scripts/test-review-storage.sh` requires a disposable database and confirms every exact test name
before execution. CI uses the same runner and guards its selection. See [project status](project-status.md)
for executed results and omitted checks. Never run fault injection against deployed data.

[C2-E](session-task-commands.md) now composes current Sessions with private manual Task commands, but
does not authorize this store. [C2-F](session-artifact-commands.md) adds private Artifact authorization,
not a grant to create Reviews. Next compose reviewer/source grants, Artifact resolution and protected
commands in one reviewed authority path. Task acceptance must check the exact Task/content/Review
revisions and applicable policy rather than merely consume a stored `approved` string. Public UI/API,
offline teaching migration, retention and reliable Run/event delivery remain separate work. C2 and
C-M2/C-M5 are not complete.
