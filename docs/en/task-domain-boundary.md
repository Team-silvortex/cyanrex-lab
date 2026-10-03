# ADR-013 Task definitions and domain assessment

Status: **C2-A included in source release 0.4.8**. Decision date: **2026-10-03**.

The platform now owns an immutable, versioned task catalogue and typed rule-dispatch boundary.
The built-in eBPF teaching pack owns the five lab definitions and their source/runtime rules.
Existing teaching reads and new-attempt assessment use this shared catalogue through a compatibility
adapter. This is a working code path, not just new names, but it is not the complete C2 Task service.

## Ownership and composition

| Location | Responsibility |
|---|---|
| `engine/src/models/collaboration/task.rs` | Definition and package references, schema/policy pins, rule outcomes |
| `engine/src/services/task_catalog.rs` | Validate registration, exact lookup, typed dispatch and bounded output |
| `engine/src/domain_packs/ebpf_teaching/` | Lab IDs, order, templates, C source evidence and attachment requirements |
| `engine/src/services/learning_catalog.rs` | Preserve the existing Rust entry points through a compatibility facade |
| `LearningStore` and existing routes | Existing history, progress, feedback, ownership and persistence |

`TaskCatalog<P>` takes an explicitly constructed, trusted `TaskProvider`. Its associated evidence type
keeps provider-specific inputs outside the platform contract. There is no central task-kind enum,
arbitrary JSON evaluator or built-in assumption that a task needs a Run. A text-only provider in tests
uses the same catalogue to assess a manually written summary without a compiler, teacher or kernel.
That fixture demonstrates the boundary; it is not a new document editor or public workflow.

The teaching adapter maps a legacy lab ID to an exact definition reference, calls the shared catalogue,
and translates `Passed` into the existing `completed` field. It never compiles or loads code. The
original source tokenizer moved unchanged into the teaching pack; comments, strings, helper-name
substrings, null guards, runtime stage and verified-attachment checks retain their previous behavior.

## Versioning and failure rules

References include the package name/version and task name/version. Evidence schema and assessment
policy references are separately pinned. These are positive JSON-safe integer contract versions, not
the product version or module manifest SemVer. The built-in package, definitions, schemas and policies
start at version 1. The task contract uses the existing core schema version 1.

Registration snapshots definitions and rejects duplicate exact identities, foreign package coordinates,
names outside the package namespace and invalid display metadata. Lookup requires the whole reference;
unknown packages, package versions, names or definition versions never fall back to a current rule.
The catalogue supplies result identity from its registration, not evaluator output or caller metadata.
The teaching provider also rejects changed definition/policy/schema metadata when called directly.

A catalogue holds one package version and 1–256 definitions; different definition versions can coexist.
Titles are nonblank, control-free and at most 256 UTF-8 bytes; summaries allow 2,048 bytes. Results allow
64 feedback messages, 2,048 bytes each and 16 KiB in total. These bound registered metadata and accepted
results, not the memory or execution cost of trusted provider code. Providers still own evidence
validation and processing limits; namespace checks do not authenticate package publishers.

Published rules must not be edited under an existing definition/policy pin. Register a new definition
and policy version when semantics change, retaining old evaluators if they must still be used. Missing
versions fail explicitly. This in-process catalogue does not persist packages or keep removed code
available across deployments; that durable lifecycle remains future work.

## Compatibility and security

The five legacy lab IDs, order, titles, summaries, document/template links, feedback text/order and
unknown-lab error remain unchanged. Attempt JSON/SQL, teacher feedback and HTTP/SDK contracts are not
extended with synthetic versions. Existing history is read as recorded, never re-assessed or assigned
an invented historical policy. Progress retains the earliest successful attempt; later failures do
not undo completion. Owner-bound resume and teacher-only review rules remain unchanged.

`TaskAssessment` is a transient automatic rule result, **not** a durable Review, human approval or Task
state change. It carries no authenticated reviewer, immutable artifact target, evidence digest or audit
receipt. Client-supplied outcomes cannot authorize acceptance. The new layer neither authenticates a
caller nor grants execution rights; the existing route/Session/CSRF boundary still applies.

No new route, UI, database schema, migration, live authentication cutover, dynamic plugin loader or
privileged execution path is introduced. Module manifest v1 discovery/start/stop still only controls
the existing declarative catalogue. A registered task provider does not acquire those capabilities.
The monolithic Engine remains privileged; source-level decoupling is not process isolation.

## Validation and next boundary

`task_catalog_tdd.rs` covers a second domain, exact-version coexistence, rejected fallback, duplicate/
foreign registration, metadata/result limits, strict decoding and forbidden domain imports.
`teaching_task_adapter_tdd.rs` freezes the legacy catalogue and feedback, compares both adapters over
180 outcome combinations, rejects substituted pins and verifies persisted history/first completion
after reload. A compile-fail example rejects document evidence passed to the teaching evaluator.
Existing template, learning-store, route/permission and module-boundary regressions remain applicable.

The functional-network checker now enumerates current lab IDs from the teaching pack, with a regression
that rejects missing source instead of reading the old facade. The dated 0.3.8 inventory/fingerprints
remain untouched; their expected drift is not a newly accepted baseline. See [project status](project-status.md)
for executed checks and limitations.

[ADR-014 / C2-B](task-instance-store.md) stores Task snapshots and atomic events through a trusted direct
API. [C2-I](session-catalog-tasks.md) now admits those exact definitions for current-Session
private Tasks: its handle copies metadata and survives provider disposal, without calling `assess`.
Reads/transitions compare the entire saved definition; changed metadata under the same pin is rejected,
and missing entries never fall back to latest. Its 0–32 exact owned inputs still require valid Artifact
metadata when empty. C2-E/G manual adapters remain definition-free. This is metadata admission, not
conversion to typed Evidence, assessment-policy execution or Task acceptance.

[C2-J](session-task-revisions.md) also checks the full configured definition when replacing
a Draft's inputs. It does not upgrade the definition or reinterpret content under a newer policy.

[C2-H](session-review-commands.md) separately authorizes private human Review of exact owned Artifacts;
it does not establish historical Session provenance or cross-user review rights. Validate offline attempt
mapping and unchanged visibility before any storage cutover. A non-teaching multi-person review workflow
and durable Run/outbox remain separate C2/C3 milestones; these slices do not claim C-M2, C-M3 or C-M5 completion.
