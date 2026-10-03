use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use cyanrex_engine::{
    models::collaboration::{
        AssessmentOutcome, AssessmentVerdict, CoreSchemaVersion, TaskDefinition, TaskDefinitionRef,
        VersionedName,
    },
    services::task_catalog::{TaskCatalog, TaskCatalogError, TaskProvider},
};

fn versioned(name: &str, version: u64) -> VersionedName {
    VersionedName {
        name: name.parse().unwrap(),
        version: version.try_into().unwrap(),
    }
}

fn definition(version: u64) -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: versioned("example.documents", 3),
            name: "example.documents.summary".parse().unwrap(),
            version: version.try_into().unwrap(),
        },
        title: "Write a summary".into(),
        summary: "A text-only task; no compiler, kernel, teacher or Run is required.".into(),
        evidence_schema: versioned("example.documents.text", 1),
        assessment_policy: versioned("example.documents.minimum_words", version),
    }
}

// An independent, typed provider exercises the actual shared service, not an eBPF enum branch.
struct Documents {
    definitions: Vec<TaskDefinition>,
    calls: Arc<AtomicUsize>,
    outcome: Option<AssessmentOutcome>,
}

impl Documents {
    fn new(definitions: Vec<TaskDefinition>) -> Self {
        Self {
            definitions,
            calls: Arc::new(AtomicUsize::new(0)),
            outcome: None,
        }
    }
}

impl TaskProvider for Documents {
    type Evidence<'a> = &'a str;

    fn package(&self) -> VersionedName {
        versioned("example.documents", 3)
    }
    fn definitions(&self) -> Vec<TaskDefinition> {
        self.definitions.clone()
    }

    fn assess(
        &self,
        definition: &TaskDefinition,
        text: &str,
    ) -> Result<AssessmentOutcome, TaskCatalogError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(outcome) = &self.outcome {
            return Ok(outcome.clone());
        }
        if text.is_empty() {
            return Err(TaskCatalogError::InvalidEvidence("missing text"));
        }
        let minimum = u64::from(definition.assessment_policy.version) as usize;
        Ok(AssessmentOutcome {
            verdict: if text.split_whitespace().count() >= minimum {
                AssessmentVerdict::Passed
            } else {
                AssessmentVerdict::NotPassed
            },
            feedback: vec![format!("Rule requires {minimum} word(s).")],
        })
    }
}

#[test]
fn non_teaching_task_uses_the_same_catalog_without_a_run() {
    let catalog = TaskCatalog::new(Documents::new(vec![definition(1)])).unwrap();
    let registered = &catalog.definitions()[0];
    let assessment = catalog
        .assess(&registered.reference, "A manually written summary.")
        .unwrap();
    assert_eq!(assessment.definition, registered.reference);
    assert_eq!(assessment.policy, registered.assessment_policy);
    assert_eq!(assessment.evidence_schema, registered.evidence_schema);
    assert_eq!(assessment.outcome.verdict, AssessmentVerdict::Passed);
    assert_eq!(catalog.get(&registered.reference), Some(registered));
}

#[test]
fn old_definition_and_policy_remain_pinned_when_a_new_version_is_present() {
    let old = definition(1);
    let new = definition(2);
    let catalog = TaskCatalog::new(Documents::new(vec![old.clone(), new.clone()])).unwrap();
    let old_assessment = catalog.assess(&old.reference, "one").unwrap();
    let new_assessment = catalog.assess(&new.reference, "one").unwrap();
    assert_eq!(old_assessment.outcome.verdict, AssessmentVerdict::Passed);
    assert_eq!(new_assessment.outcome.verdict, AssessmentVerdict::NotPassed);
    assert_eq!(old_assessment.policy.version, 1.try_into().unwrap());
    assert_eq!(new_assessment.policy.version, 2.try_into().unwrap());
    assert_eq!(catalog.definitions(), &[old, new]);
}

#[test]
fn unknown_package_definition_and_version_never_dispatch_or_fall_back() {
    let known = definition(1);
    let provider = Documents::new(vec![known.clone()]);
    let calls = provider.calls.clone();
    let catalog = TaskCatalog::new(provider).unwrap();
    let mut wrong_package = known.reference.clone();
    wrong_package.package.name = "other.documents".parse().unwrap();
    let mut wrong_package_version = known.reference.clone();
    wrong_package_version.package.version = 4.try_into().unwrap();
    let mut wrong_name = known.reference.clone();
    wrong_name.name = "example.documents.unknown".parse().unwrap();
    for reference in [
        wrong_package,
        wrong_package_version,
        wrong_name,
        definition(2).reference,
    ] {
        assert!(catalog.get(&reference).is_none());
        assert_eq!(
            catalog.assess(&reference, "text"),
            Err(TaskCatalogError::UnknownDefinition)
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn duplicate_definition_identity_cannot_replace_a_rule() {
    let original = definition(1);
    let mut replacement = original.clone();
    replacement.assessment_policy.version = 2.try_into().unwrap();
    assert!(TaskCatalog::new(Documents::new(vec![original, replacement])).is_err());
}

#[test]
fn foreign_and_similar_prefix_names_cannot_claim_package_ownership() {
    for name in ["other.documents.rule", "example.documents_extra.rule"] {
        for field in 0..3 {
            let mut invalid = definition(1);
            match field {
                0 => invalid.reference.name = name.parse().unwrap(),
                1 => invalid.evidence_schema.name = name.parse().unwrap(),
                _ => invalid.assessment_policy.name = name.parse().unwrap(),
            }
            assert!(TaskCatalog::new(Documents::new(vec![invalid])).is_err());
        }
    }
    let mut invalid = definition(1);
    invalid.reference.package.version = 2.try_into().unwrap();
    assert!(TaskCatalog::new(Documents::new(vec![invalid])).is_err());
}

#[test]
fn catalog_metadata_and_count_are_bounded() {
    assert!(TaskCatalog::new(Documents::new(vec![])).is_err());
    for count in [256, 257] {
        let definitions = (1..=count).map(definition).collect();
        assert_eq!(
            TaskCatalog::new(Documents::new(definitions)).is_ok(),
            count == 256
        );
    }
    for title in [
        "".into(),
        " \t".into(),
        "x".repeat(257),
        "line\nbreak".into(),
    ] {
        let mut invalid = definition(1);
        invalid.title = title;
        assert!(TaskCatalog::new(Documents::new(vec![invalid])).is_err());
    }
    let mut invalid = definition(1);
    invalid.summary = "x".repeat(2049);
    assert!(TaskCatalog::new(Documents::new(vec![invalid])).is_err());
}

#[test]
fn invalid_evidence_and_oversized_provider_results_are_not_success() {
    let reference = definition(1).reference;
    let catalog = TaskCatalog::new(Documents::new(vec![definition(1)])).unwrap();
    assert_eq!(
        catalog.assess(&reference, ""),
        Err(TaskCatalogError::InvalidEvidence("missing text"))
    );
    for feedback in [
        vec!["x".repeat(2049)],
        vec!["x".into(); 65],
        vec!["x".repeat(2048); 9],
    ] {
        let mut provider = Documents::new(vec![definition(1)]);
        provider.outcome = Some(AssessmentOutcome {
            verdict: AssessmentVerdict::Passed,
            feedback,
        });
        let catalog = TaskCatalog::new(provider).unwrap();
        assert_eq!(
            catalog.assess(&reference, "text"),
            Err(TaskCatalogError::InvalidAssessment)
        );
    }
}

#[test]
fn definition_wire_contract_is_strict_and_independent_of_product_version() {
    let original = definition(2);
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(
        serde_json::from_value::<TaskDefinition>(json.clone()).unwrap(),
        original
    );
    assert_eq!(json["reference"]["package"]["version"], 3);
    assert_eq!(json["reference"]["version"], 2);
    for (pointer, value) in [
        ("/schema_version", serde_json::json!(2)),
        ("/reference/version", serde_json::json!(0)),
        (
            "/assessment_policy/version",
            serde_json::json!(9_007_199_254_740_992_u64),
        ),
        ("/evidence_schema/name", serde_json::json!("unqualified")),
    ] {
        let mut invalid = json.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(serde_json::from_value::<TaskDefinition>(invalid).is_err());
    }
    let mut invalid = json;
    invalid["teacher"] = serde_json::json!(true);
    assert!(serde_json::from_value::<TaskDefinition>(invalid).is_err());
}

#[test]
fn shared_catalog_and_contracts_do_not_import_domain_or_execution_services() {
    for source in [
        include_str!("../src/models/collaboration/task.rs"),
        include_str!("../src/services/task_catalog.rs"),
    ] {
        for forbidden in [
            "::domain_packs",
            "::learning",
            "ebpf",
            "runner_",
            "AuthRole",
            "LabRunEvidence",
        ] {
            assert!(
                !source.contains(forbidden),
                "domain dependency leaked into core: {forbidden}"
            );
        }
    }
}
