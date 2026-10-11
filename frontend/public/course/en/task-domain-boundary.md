# ADR-013 Task definitions and domain assessment

Source release **0.5.5** (**2026-10-11**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

Status: **C2-A included in source release 0.4.8**. Decision date: **2026-10-03**.

The platform now owns an immutable, versioned task catalogue and typed rule-dispatch boundary.
The built-in eBPF teaching pack owns the five lab definitions and their source/runtime rules.
Existing teaching reads and new-attempt assessment use this shared catalogue through a compatibility
adapter. This is a working code path, not just new names, but it is not the complete C2 Task service.

The **2026-10-08 Unreleased follow-up**, on source version 0.5.4, separates frozen definition metadata
from executable assessment. Metadata-only Task construction and Session catalogue configuration no
longer require a provider; the typed evaluator and existing authorization boundaries remain unchanged.

## Ownership and composition

| Location | Responsibility |
|---|---|
| `engine/src/models/collaboration/task.rs` | Definition and package references, schema/policy pins, rule outcomes |
| `engine/src/services/task_catalog/definitions.rs` | Frozen metadata registration and exact lookup without an evaluator |
| `engine/src/services/task_catalog.rs` | Compose the metadata catalogue with typed dispatch and bounded assessment output |
| `engine/src/domain_packs/ebpf_teaching/` | Lab IDs, order, templates, C source evidence and attachment requirements |
| `engine/src/services/learning_catalog.rs` | Preserve the existing Rust entry points through a compatibility facade |
| `LearningStore` and existing routes | Existing history, progress, feedback, ownership and persistence |

`TaskDefinitionCatalog::new(package, definitions)` validates one explicit package version and 1–256
definitions using the existing duplicate, namespace and display-metadata rules. This non-generic,
cloneable handle shares an immutable `Arc` snapshot; it retains no provider, evidence type or callable
assessment code. Exact lookup still has no latest-version fallback. It is trusted configuration, not
request deserialization, package discovery, publisher authentication or a dynamic policy registry.

The **2026-10-11 Unreleased follow-up**, still on source version 0.5.4, adds
`TaskDefinitionCatalog::compose(catalogs: &[Self]) -> Result<Self, TaskCatalogError>` for explicit
multi-package configuration. It accepts 1–256 validated metadata handles and counts their flattened
definitions before copying; the result must contain 1–256 definitions, including when an input was
already composed. That global limit also bounds every package's entries and the number of distinct
packages; there is no separate smaller package limit. Definitions retain input-catalogue order and
each input's entry order in a new frozen `Arc` snapshot.

Disjoint registrations for the same package/version can be combined. Different packages, package
versions and task-definition versions can coexist. Every definition is revalidated against its own
package coordinates. Repeating a complete definition reference is an error even when both metadata
bodies are identical: composition neither deduplicates nor overwrites and lookup still has no
latest-version fallback. It combines metadata only, not providers, evidence types, package discovery
or publisher trust. Existing single-package `new` and typed `TaskCatalog<P>` construction stay strict.

`TaskCatalog<P>` wraps that metadata handle and an explicitly constructed, trusted `TaskProvider`.
`definition_catalog()` exposes metadata only. The provider's associated evidence type keeps its
inputs outside the platform contract. There is no central task-kind enum,
arbitrary JSON evaluator or built-in assumption that a task needs a Run. A text-only provider in tests
uses the same catalogue to assess a manually written summary without a compiler, teacher or kernel.
That fixture demonstrates the boundary; it is not a new document editor or public workflow.

`TaskDraft::from_definitions` and `SessionCatalogTaskWorkspace::from_definitions` accept the metadata
handle without constructing an evaluator. Existing `TaskDraft::from_catalog` and workspace `new`
delegate to these paths for compatibility. Obtaining or cloning definitions does not grant typed
`Evidence`, `assess`, Session authority, execution or Task acceptance.

The **2026-10-08 Unreleased catalogue-content follow-up** also composes this metadata with the existing
text manifest, without tying content to an evaluator. `TaskContentStore::from_definitions` and
`SessionCatalogTaskContentWorkspace::from_definitions` explicitly select catalogue admission; creation
copies an exact full definition, and later operations reject missing or changed configured metadata.
The instance title and text labels remain separate from the definition's display metadata. Required
evidence-schema and assessment-policy pins are preserved, not executed. The original manual content
entry points remain definition-free, and storage stays at schema 3 without migration. See
[content storage](task-content-store.md) and [Session content commands](session-task-content.md) for
the strict mode boundary; this does not connect catalogue content to HTTP, browser saving or draft
publication workflows.

The teaching adapter maps a legacy lab ID to an exact definition reference, calls the shared catalogue,
and translates `Passed` into the existing `completed` field. It never compiles or loads code. The
original source tokenizer moved unchanged into the teaching pack; comments, strings, helper-name
substrings, null guards, runtime stage and verified-attachment checks retain their previous behavior.

## Definitions without automatic assessment bindings included in 0.5.5

The separate **2026-10-11 Unreleased assessment-binding follow-up**, still on source 0.5.4, lets a
definition explicitly omit automatic assessment. This is unreleased source capability, not a claim
about the published 0.5.4 tag. `TaskDefinition.evidence_schema` and
`assessment_policy` are paired `Option<VersionedName>` values: both are present for assessed
definitions, or both are `None` for definitions without an automatic rule binding. Half-bound
definitions are rejected. A frozen catalogue can contain both forms, including across explicitly
composed packages. Names, exact references, display limits and the global definition budget remain
unchanged.

Both JSON fields remain required. An unbound definition must contain explicit `null` for each;
missing fields, a single null or malformed present references are rejected rather than defaulted.
Already-bound JSON keeps its existing field order and values, so existing records need no migration.
The Rust fields have changed from `VersionedName` to `Option<VersionedName>`, requiring internal
source callers to adapt. New double-null definitions require the new reader: old binaries reject
them, and backward reading or downgrade compatibility for new unbound records is not promised.
Neither core nor storage schema numbers change.

This does not make an unbound catalogue Task a manual Task: it still stores `Some(TaskDefinition)`
with its exact package/task identity. The existing draft, schema 2 Session catalogue and schema 3
content constructors accept either admitted form without evaluating content. Reads and mutations
still compare the full definition; adding or removing bindings under the same reference is metadata
drift and is rejected in both directions. Intentional binding changes need a new definition version,
not an edit or reinterpretation of stored work.

`TaskCatalog<P>::new` continues to require both bindings for every typed-provider entry. `assess`
also checks them before calling the provider, and `TaskAssessment` keeps mandatory schema/policy
references. Absence is not a synthetic assessment, automatic approval, human Review issuance or
Task acceptance. This follow-up adds no provider discovery, public command, editor connection,
authorization branch, migration or Review/execution workflow.

## Catalogue content HTTP composition included in 0.5.5

The separate **2026-10-11 Unreleased C2-Y follow-up**, on source 0.5.4 outside the published tag,
adds [an explicit catalogue-content HTTP adapter](catalog-task-content-http.md). Trusted configuration
supplies the frozen catalogue workspace; creation accepts only an exact `definition_ref` and manifest,
never a full client definition or policy. The four commands preserve complete-definition equality,
old/new text verification and current Session authority. Bound and unbound entries both remain
catalogue Tasks without provider dispatch, Review or acceptance.

This does not extend the manual HTTP request shape or C2-P–W publication workflows. The catalogue
router is separately constructed and unmounted, with no discovery, client mode selection, automatic
merge, Session issuance, OpenAPI/SDK/browser connection or schema migration. Its new sources do not
inherit the earlier catalogue or manual-HTTP execution evidence.

## Catalogue draft publication included in 0.5.5

The independent **2026-10-11 Unreleased C2-Z follow-up**, on source 0.5.4 outside the published tag,
adds [a separate catalogue publication attempt](session-catalog-task-draft-publication.md). Pure
preparation resolves an exact trusted reference and freezes its complete metadata before target
allocation; unknown references are `InvalidPreparation`, without I/O or a latest/manual fallback.
Bound and unbound definitions both remain catalogue work. Each explicit advance performs one
existing Session Artifact publication or the final catalogue content create; confirmation requires
`Some(frozen_complete_definition)` and the expected Draft/manifest, not a provider result.

The manual `None` confirmation boundary and C2-P–W attempt/checkpoint/journal types remain unchanged.
The new opaque attempt cannot convert into them or observe, checkpoint, journal, restore or retry.
Sharing two pure consistency checks does not share authority, workflow state or recovery. This adds
no HTTP, issuer, assessment/Review, public SDK/browser, schema migration or old-evidence inheritance.

## Catalogue target observation included in 0.5.5

The independent **2026-10-11 Unreleased C2-AA follow-up**, on source 0.5.4 outside the published tag,
adds [read-only observation](session-catalog-task-draft-observation.md) to a surviving C2-Z catalogue
attempt. Current catalogue reads validate the whole stored definition before comparing it with the
frozen planned definition and ordered text. A valid different registered definition can differ;
same-reference metadata/binding drift or a manual `None` record is a typed error, not a difference.
The original Session fingerprint is not authority, and observations never become assessment, Task
acceptance, confirmed progress, recovery or retry. Public/browser composition and durable definition
lifecycle remain separate work.

## Versioning and failure rules

References include the package name/version and task name/version. Bound definitions separately pin
both evidence schema and assessment policy; explicitly unbound definitions bind neither contract.
Present pins use positive JSON-safe integer contract versions, not
the product version or module manifest SemVer. The built-in package, definitions, schemas and policies
start at version 1. The task contract uses the existing core schema version 1.

Registration snapshots definitions and rejects duplicate exact identities, foreign package coordinates,
names outside the package namespace and invalid display metadata. Lookup requires the whole reference;
unknown packages, package versions, names or definition versions never fall back to a current rule.
The catalogue supplies result identity from its registration, not evaluator output or caller metadata.
The teaching provider also rejects changed definition/policy/schema metadata when called directly.

Each `new` registration holds one package version and 1–256 definitions; different definition versions
can coexist. Explicit composition retains the same 256-definition global bound across packages.
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

The metadata-only follow-up adds `task_definition_catalog_tdd.rs` and the Session catalogue
`metadata.rs` cases for provider-free construction, frozen ownership, exact lookup and unchanged
authorization/storage behavior. Their execution evidence is recorded separately in project status.
The separate catalogue-content cases in `task_content_store` and `session_task_content` cover this
composition's mode separation, complete-definition checks and unchanged content authorization.
The multi-package follow-up adds `task_definition_catalog/composition.rs` for pure composition,
exact-reference collisions, order and nested global limits. The Session catalogue and content
composition cases verify the same metadata handle through their existing schema 2 and schema 3
commands; `taskCatalogComposition.test.mjs` guards the bounded, provider-free source boundary.
The separate `task_definition_catalog/assessment_binding.rs` cases cover explicit paired absence,
required nullable JSON fields, legacy bound encoding, mixed metadata and typed-provider refusal.
Session catalogue/content binding cases check stored null pairs and rejection of same-pin binding
drift; `taskAssessmentBinding.test.mjs` guards that source boundary and the exact runner inventory.

The functional-network checker now enumerates current lab IDs from the teaching pack, with a regression
that rejects missing source instead of reading the old facade. The dated 0.3.8 inventory/fingerprints
remain untouched; their expected drift is not a newly accepted baseline. See [project status](project-status.md)
for executed checks and limitations.

[ADR-014 / C2-B](task-instance-store.md) stores Task snapshots and atomic events through a trusted direct
API. [C2-I](session-catalog-tasks.md) now admits those exact definitions for current-Session
private Tasks: its handle retains frozen shared metadata and survives provider disposal, without calling `assess`.
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
