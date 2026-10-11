use cyanrex_engine::{
    models::collaboration::{
        ArtifactRef, AssessmentOutcome, AssessmentVerdict, CoreSchemaVersion, TaskDefinition,
        TaskDefinitionRef, VersionedName, WorkspaceRef,
    },
    services::{
        task_catalog::{TaskCatalog, TaskCatalogError, TaskDefinitionCatalog, TaskProvider},
        task_store::{TaskDraft, TaskStoreError},
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[path = "task_definition_catalog/assessment_binding.rs"]
mod assessment_binding;
#[path = "task_definition_catalog/composition.rs"]
mod composition;

fn pin(name: &str, version: u64) -> VersionedName {
    VersionedName {
        name: name.parse().unwrap(),
        version: version.try_into().unwrap(),
    }
}

fn definition(version: u64) -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: pin("example.editorial", 3),
            name: "example.editorial.prepare".parse().unwrap(),
            version: version.try_into().unwrap(),
        },
        title: "Prepare a document".into(),
        summary: "Metadata registration does not require an evaluator.".into(),
        evidence_schema: Some(pin("example.editorial.text", 1)),
        assessment_policy: Some(pin("example.editorial.review", version)),
    }
}

#[test]
fn metadata_catalog_registers_and_resolves_without_a_task_provider() {
    let first = definition(1);
    let next = definition(2);
    let catalog = TaskDefinitionCatalog::new(
        first.reference.package.clone(),
        vec![first.clone(), next.clone()],
    )
    .unwrap();
    assert_eq!(catalog.definitions(), &[first.clone(), next.clone()]);
    assert_eq!(catalog.get(&first.reference), Some(&first));
    assert_eq!(catalog.get(&next.reference), Some(&next));
    assert!(catalog.get(&definition(3).reference).is_none());
}

#[test]
fn metadata_catalog_clones_share_a_frozen_owned_snapshot() {
    let original = definition(1);
    let mut supplied = vec![original.clone()];
    let catalog =
        TaskDefinitionCatalog::new(original.reference.package.clone(), supplied.clone()).unwrap();
    supplied[0].summary = "Changed source metadata".into();
    let copied = catalog.clone();
    fn send_sync(_: &(impl Send + Sync)) {}
    send_sync(&copied);
    assert!(std::ptr::eq(
        catalog.definitions().as_ptr(),
        copied.definitions().as_ptr()
    ));
    drop((catalog, supplied));
    assert_eq!(copied.definitions(), &[original.clone()]);
    assert_eq!(copied.get(&original.reference), Some(&original));
}

#[test]
fn metadata_catalog_keeps_exact_single_package_count_and_display_limits() {
    let package = definition(1).reference.package;
    assert!(TaskDefinitionCatalog::new(package.clone(), vec![]).is_err());
    for count in [1, 256, 257] {
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), (1..=count).map(definition).collect())
                .is_ok(),
            count <= 256
        );
    }
    for (title, valid) in [
        ("x".repeat(256), true),
        ("é".repeat(128), true),
        ("x".repeat(257), false),
        ("é".repeat(129), false),
        (String::new(), false),
        (" \t".into(), false),
        ("a\u{0085}b".into(), false),
    ] {
        let mut changed = definition(1);
        changed.title = title;
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), vec![changed]).is_ok(),
            valid
        );
    }
    for (summary, valid) in [
        ("x".repeat(2048), true),
        ("é".repeat(1024), true),
        ("x".repeat(2049), false),
        ("Text\nwith a summary line break".into(), true),
    ] {
        let mut changed = definition(1);
        changed.summary = summary;
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), vec![changed]).is_ok(),
            valid
        );
    }
}

#[test]
fn metadata_catalog_rejects_duplicate_pins_and_foreign_names_without_reinterpreting_them() {
    let original = definition(1);
    let package = original.reference.package.clone();
    let mut same_pin = original.clone();
    same_pin.assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap();
    assert!(TaskDefinitionCatalog::new(package.clone(), vec![original, same_pin]).is_err());
    for foreign in ["other.editorial.item", "example.editorial_extra.item"] {
        for field in 0..3 {
            let mut changed = definition(1);
            match field {
                0 => changed.reference.name = foreign.parse().unwrap(),
                1 => changed.evidence_schema.as_mut().unwrap().name = foreign.parse().unwrap(),
                2 => changed.assessment_policy.as_mut().unwrap().name = foreign.parse().unwrap(),
                _ => unreachable!(),
            }
            assert!(TaskDefinitionCatalog::new(package.clone(), vec![changed]).is_err());
        }
    }
    for changed_package in [pin("example.editorial", 4), pin("other.editorial", 3)] {
        let mut changed = definition(1);
        changed.reference.package = changed_package;
        assert!(TaskDefinitionCatalog::new(package.clone(), vec![changed]).is_err());
    }
}

#[test]
fn metadata_lookup_never_falls_back_across_any_reference_coordinate() {
    let known = definition(1);
    let catalog =
        TaskDefinitionCatalog::new(known.reference.package.clone(), vec![known.clone()]).unwrap();
    for field in 0..4 {
        let mut unknown = known.reference.clone();
        match field {
            0 => unknown.package.name = "other.editorial".parse().unwrap(),
            1 => unknown.package.version = 4.try_into().unwrap(),
            2 => unknown.name = "example.editorial.unknown".parse().unwrap(),
            3 => unknown.version = 2.try_into().unwrap(),
            _ => unreachable!(),
        }
        assert!(catalog.get(&unknown).is_none());
    }
    assert_eq!(catalog.get(&known.reference), Some(&known));
}

#[derive(Default)]
struct Probe {
    package_calls: Cell<usize>,
    definition_calls: Cell<usize>,
    assessments: Cell<usize>,
    drops: Cell<usize>,
}

struct Rules {
    definitions: Rc<RefCell<Vec<TaskDefinition>>>,
    probe: Rc<Probe>,
}

impl Drop for Rules {
    fn drop(&mut self) {
        self.probe.drops.set(self.probe.drops.get() + 1);
    }
}

impl TaskProvider for Rules {
    type Evidence<'a> = &'a str;
    fn package(&self) -> VersionedName {
        self.probe
            .package_calls
            .set(self.probe.package_calls.get() + 1);
        pin("example.editorial", 3)
    }
    fn definitions(&self) -> Vec<TaskDefinition> {
        self.probe
            .definition_calls
            .set(self.probe.definition_calls.get() + 1);
        self.definitions.borrow().clone()
    }
    fn assess(
        &self,
        _: &TaskDefinition,
        evidence: &str,
    ) -> Result<AssessmentOutcome, TaskCatalogError> {
        self.probe.assessments.set(self.probe.assessments.get() + 1);
        Ok(AssessmentOutcome {
            verdict: if evidence.is_empty() {
                AssessmentVerdict::NotPassed
            } else {
                AssessmentVerdict::Passed
            },
            feedback: vec!["Typed provider result".into()],
        })
    }
}

fn rules() -> (
    TaskCatalog<Rules>,
    Rc<Probe>,
    Rc<RefCell<Vec<TaskDefinition>>>,
) {
    let definitions = Rc::new(RefCell::new(vec![definition(1)]));
    let probe = Rc::new(Probe::default());
    let catalog = TaskCatalog::new(Rules {
        definitions: definitions.clone(),
        probe: probe.clone(),
    })
    .unwrap();
    (catalog, probe, definitions)
}

#[test]
fn extracted_metadata_outlives_non_send_provider_without_dispatching_or_resnapshotting() {
    let (catalog, probe, source) = rules();
    let metadata = catalog.definition_catalog().clone();
    assert!(std::ptr::eq(
        catalog.definitions().as_ptr(),
        metadata.definitions().as_ptr()
    ));
    source.borrow_mut()[0].title = "Provider mutated after registration".into();
    assert_eq!(metadata.definitions(), &[definition(1)]);
    let result = catalog
        .assess(&definition(1).reference, "Evidence")
        .unwrap();
    assert_eq!(result.definition, definition(1).reference);
    assert_eq!(result.policy, definition(1).assessment_policy.unwrap());
    assert_eq!(
        result.evidence_schema,
        definition(1).evidence_schema.unwrap()
    );
    assert_eq!(result.outcome.verdict, AssessmentVerdict::Passed);
    drop(catalog);
    assert_eq!(probe.drops.get(), 1);
    assert_eq!(probe.package_calls.get(), 1);
    assert_eq!(probe.definition_calls.get(), 1);
    assert_eq!(probe.assessments.get(), 1);
    assert_eq!(metadata.get(&definition(1).reference), Some(&definition(1)));
    assert!(TaskDraft::from_definitions(
        &metadata,
        &definition(1).reference,
        "Detached metadata",
        vec![]
    )
    .is_ok());
    assert_eq!(probe.assessments.get(), 1);
}

fn artifact() -> ArtifactRef {
    ArtifactRef {
        workspace: WorkspaceRef {
            authority_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".parse().unwrap(),
            workspace_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".parse().unwrap(),
        },
        artifact_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".parse().unwrap(),
        revision_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd".parse().unwrap(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}

#[test]
fn provider_free_drafts_preserve_the_legacy_constructor_admission_contract() {
    let (catalog, probe, _) = rules();
    let definitions = catalog.definition_catalog();
    let known = definition(1).reference;
    for title in ["Standalone", "é", "A Task with optional code"] {
        assert!(TaskDraft::from_definitions(definitions, &known, title, vec![]).is_ok());
        assert!(TaskDraft::from_catalog(&catalog, &known, title, vec![]).is_ok());
    }
    for title in [
        String::new(),
        " \t".into(),
        "x".repeat(257),
        "line\nbreak".into(),
    ] {
        assert!(matches!(
            TaskDraft::from_definitions(definitions, &known, &title, vec![]),
            Err(TaskStoreError::InvalidInput)
        ));
        assert!(matches!(
            TaskDraft::from_catalog(&catalog, &known, &title, vec![]),
            Err(TaskStoreError::InvalidInput)
        ));
    }
    for inputs in [vec![artifact(), artifact()], vec![artifact(); 33]] {
        assert!(matches!(
            TaskDraft::from_definitions(definitions, &known, "Invalid inputs", inputs.clone()),
            Err(TaskStoreError::InvalidInput)
        ));
        assert!(matches!(
            TaskDraft::from_catalog(&catalog, &known, "Invalid inputs", inputs),
            Err(TaskStoreError::InvalidInput)
        ));
    }
    let unknown = definition(2).reference;
    assert!(matches!(
        TaskDraft::from_definitions(definitions, &unknown, "Missing", vec![]),
        Err(TaskStoreError::UnknownDefinition)
    ));
    assert!(matches!(
        TaskDraft::from_catalog(&catalog, &unknown, "Missing", vec![]),
        Err(TaskStoreError::UnknownDefinition)
    ));
    assert_eq!(probe.assessments.get(), 0);
}

#[test]
fn definition_wire_versions_remain_strict_before_metadata_registration() {
    let original = definition(1);
    let raw = serde_json::to_value(&original).unwrap();
    for pointer in [
        "/reference/package/version",
        "/reference/version",
        "/evidence_schema/version",
        "/assessment_policy/version",
    ] {
        for invalid in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("1"),
            serde_json::json!(9_007_199_254_740_992_u64),
        ] {
            let mut altered = raw.clone();
            *altered.pointer_mut(pointer).unwrap() = invalid;
            assert!(serde_json::from_value::<TaskDefinition>(altered).is_err());
        }
    }
    let mut future = raw;
    future["schema_version"] = serde_json::json!(2);
    assert!(serde_json::from_value::<TaskDefinition>(future).is_err());
    let mut largest = original.clone();
    largest.reference.version = 9_007_199_254_740_991_u64.try_into().unwrap();
    assert!(TaskDefinitionCatalog::new(original.reference.package, vec![largest]).is_ok());
}
