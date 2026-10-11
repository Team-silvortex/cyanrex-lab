use super::*;
use cyanrex_engine::models::collaboration::TaskAssessment;

fn unassessed(version: u64) -> TaskDefinition {
    let mut value = definition(version);
    value.evidence_schema = None;
    value.assessment_policy = None;
    value
}

fn registered(values: Vec<TaskDefinition>) -> TaskDefinitionCatalog {
    TaskDefinitionCatalog::new(values[0].reference.package.clone(), values).unwrap()
}

#[test]
fn unassessed_metadata_registers_composes_and_constructs_catalogue_drafts() {
    let manual_work = unassessed(1);
    let assessed_work = definition(2);
    let mixed = registered(vec![manual_work.clone(), assessed_work.clone()]);
    assert_eq!(
        mixed.definitions(),
        &[manual_work.clone(), assessed_work.clone()]
    );
    let composed = TaskDefinitionCatalog::compose(&[
        registered(vec![manual_work.clone()]),
        registered(vec![assessed_work.clone()]),
    ])
    .unwrap();
    assert_eq!(composed.definitions(), mixed.definitions());
    for value in [manual_work, assessed_work] {
        assert_eq!(composed.get(&value.reference), Some(&value));
        assert!(
            TaskDraft::from_definitions(&composed, &value.reference, "Private work", vec![])
                .is_ok()
        );
    }
    assert!(composed.get(&definition(3).reference).is_none());
}

#[test]
fn definition_binding_fields_are_required_even_when_both_values_are_null() {
    for original in [definition(1), unassessed(1)] {
        let raw = serde_json::to_value(original).unwrap();
        for missing in [
            vec!["evidence_schema"],
            vec!["assessment_policy"],
            vec!["evidence_schema", "assessment_policy"],
        ] {
            let mut altered = raw.clone();
            for field in &missing {
                altered.as_object_mut().unwrap().remove(*field);
            }
            assert!(
                serde_json::from_value::<TaskDefinition>(altered).is_err(),
                "omitted binding fields must not become None: {missing:?}"
            );
        }
    }
}

#[test]
fn assessed_definition_json_keeps_exact_legacy_shape() {
    let expected = concat!(
        "{\"schema_version\":1,\"reference\":{\"package\":{\"name\":\"example.editorial\",\"version\":3},",
        "\"name\":\"example.editorial.prepare\",\"version\":1},",
        "\"title\":\"Prepare a document\",\"summary\":\"Metadata registration does not require an evaluator.\",",
        "\"evidence_schema\":{\"name\":\"example.editorial.text\",\"version\":1},",
        "\"assessment_policy\":{\"name\":\"example.editorial.review\",\"version\":1}}"
    );
    assert_eq!(serde_json::to_string(&definition(1)).unwrap(), expected);
    assert_eq!(
        serde_json::from_str::<TaskDefinition>(expected).unwrap(),
        definition(1)
    );
}

#[test]
fn explicit_unassessed_nulls_and_legacy_positional_decoding_remain_unambiguous() {
    for original in [definition(1), unassessed(1)] {
        let raw = serde_json::to_value(&original).unwrap();
        if original.evidence_schema.is_none() {
            assert!(raw.as_object().unwrap().contains_key("evidence_schema"));
            assert!(raw.as_object().unwrap().contains_key("assessment_policy"));
            assert_eq!(raw["evidence_schema"], serde_json::Value::Null);
            assert_eq!(raw["assessment_policy"], serde_json::Value::Null);
        }
        assert_eq!(
            serde_json::from_value::<TaskDefinition>(raw.clone()).unwrap(),
            original
        );
        let fields = [
            "schema_version",
            "reference",
            "title",
            "summary",
            "evidence_schema",
            "assessment_policy",
        ];
        let positions = fields
            .iter()
            .map(|field| raw[*field].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::from_value::<TaskDefinition>(serde_json::Value::Array(positions.clone()))
                .unwrap(),
            original
        );
        for length in [4, 5] {
            assert!(
                serde_json::from_value::<TaskDefinition>(serde_json::Value::Array(
                    positions[..length].to_vec()
                ))
                .is_err()
            );
        }
        let bound = serde_json::to_value(definition(1)).unwrap();
        for (index, field) in [(4, "evidence_schema"), (5, "assessment_policy")] {
            let mut half = positions.clone();
            half[index] = if half[index].is_null() {
                bound[field].clone()
            } else {
                serde_json::Value::Null
            };
            assert!(
                serde_json::from_value::<TaskDefinition>(serde_json::Value::Array(half)).is_err()
            );
        }
        let mut extra = positions;
        extra.push(serde_json::Value::Null);
        assert!(serde_json::from_value::<TaskDefinition>(serde_json::Value::Array(extra)).is_err());
    }
}

#[test]
fn half_bindings_duplicate_fields_and_unknown_fields_never_decode_as_definitions() {
    let original = definition(1);
    let raw = serde_json::to_value(&original).unwrap();
    for field in ["evidence_schema", "assessment_policy"] {
        let mut half = raw.clone();
        half[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<TaskDefinition>(half).is_err());
    }
    for original in [original, unassessed(1)] {
        let encoded = serde_json::to_string(&original).unwrap();
        for field in ["evidence_schema", "assessment_policy"] {
            let marker = format!("\"{field}\":");
            let duplicate = encoded.replace(&marker, &format!("{marker}null,{marker}"));
            assert!(serde_json::from_str::<TaskDefinition>(&duplicate).is_err());
        }
        let mut unknown = serde_json::to_value(original).unwrap();
        unknown["assessment_binding"] = serde_json::json!(null);
        assert!(serde_json::from_value::<TaskDefinition>(unknown).is_err());
    }
}

#[test]
fn optional_pins_do_not_loosen_names_versions_or_nested_record_decoding() {
    let raw = serde_json::to_value(definition(1)).unwrap();
    for field in ["evidence_schema", "assessment_policy"] {
        for invalid in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("1"),
            serde_json::json!(9_007_199_254_740_992_u64),
        ] {
            let mut altered = raw.clone();
            altered[field]["version"] = invalid;
            assert!(serde_json::from_value::<TaskDefinition>(altered).is_err());
        }
        for name in ["unqualified", "Example.editorial.pin", "example..pin"] {
            let mut altered = raw.clone();
            altered[field]["name"] = serde_json::json!(name);
            assert!(serde_json::from_value::<TaskDefinition>(altered).is_err());
        }
        for invalid in [
            serde_json::json!({"name":"example.editorial.pin"}),
            serde_json::json!({"version":1}),
            serde_json::json!({"name":"example.editorial.pin","version":1,"latest":true}),
            serde_json::json!(false),
            serde_json::json!("none"),
        ] {
            let mut altered = raw.clone();
            altered[field] = invalid;
            assert!(serde_json::from_value::<TaskDefinition>(altered).is_err());
        }
    }
}

#[test]
fn runtime_admission_rejects_half_bindings_and_keeps_metadata_bounds_and_namespaces() {
    let package = definition(1).reference.package;
    for field in 0..2 {
        let mut half = definition(1);
        if field == 0 {
            half.evidence_schema = None;
        } else {
            half.assessment_policy = None;
        }
        assert!(TaskDefinitionCatalog::new(package.clone(), vec![half]).is_err());
    }
    for field in 0..3 {
        let mut invalid = definition(1);
        match field {
            0 => invalid.reference.name = "other.editorial.prepare".parse().unwrap(),
            1 => {
                invalid.evidence_schema.as_mut().unwrap().name =
                    "other.editorial.text".parse().unwrap()
            }
            2 => {
                invalid.assessment_policy.as_mut().unwrap().name =
                    "other.editorial.review".parse().unwrap()
            }
            _ => unreachable!(),
        }
        assert!(TaskDefinitionCatalog::new(package.clone(), vec![invalid]).is_err());
    }
    for name in ["other.editorial.prepare", "example.editorial_extra.prepare"] {
        let mut invalid = unassessed(1);
        invalid.reference.name = name.parse().unwrap();
        assert!(TaskDefinitionCatalog::new(package.clone(), vec![invalid]).is_err());
    }
    for foreign_package in [pin("other.editorial", 3), pin("example.editorial", 4)] {
        let mut invalid = unassessed(1);
        invalid.reference.package = foreign_package;
        assert!(TaskDefinitionCatalog::new(package.clone(), vec![invalid]).is_err());
    }
    for (title, valid) in [
        ("x".repeat(256), true),
        ("é".repeat(128), true),
        ("x".repeat(257), false),
        ("é".repeat(129), false),
        ("".into(), false),
        ("line\nbreak".into(), false),
    ] {
        let mut value = unassessed(1);
        value.title = title;
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), vec![value]).is_ok(),
            valid
        );
    }
    for (summary, valid) in [
        ("x".repeat(2048), true),
        ("é".repeat(1024), true),
        ("x".repeat(2049), false),
        ("é".repeat(1025), false),
    ] {
        let mut value = unassessed(1);
        value.summary = summary;
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), vec![value]).is_ok(),
            valid
        );
    }
    for count in [256, 257] {
        assert_eq!(
            TaskDefinitionCatalog::new(package.clone(), (1..=count).map(unassessed).collect())
                .is_ok(),
            count == 256
        );
    }
}

#[test]
fn exact_pin_duplicates_cannot_swap_between_bound_and_unassessed_metadata() {
    let bound = definition(1);
    let inert = unassessed(1);
    assert_ne!(bound, inert);
    let package = bound.reference.package.clone();
    for values in [
        vec![bound.clone(), inert.clone()],
        vec![inert.clone(), bound.clone()],
        vec![inert.clone(), inert.clone()],
    ] {
        assert!(matches!(
            TaskDefinitionCatalog::new(package.clone(), values),
            Err(TaskCatalogError::InvalidCatalog("duplicate definition"))
        ));
    }
    let bound_catalog = registered(vec![bound.clone()]);
    let inert_catalog = registered(vec![inert.clone()]);
    assert_eq!(bound_catalog.get(&bound.reference), Some(&bound));
    assert_eq!(inert_catalog.get(&inert.reference), Some(&inert));
    for sources in [
        vec![bound_catalog.clone(), inert_catalog.clone()],
        vec![inert_catalog.clone(), bound_catalog],
        vec![inert_catalog.clone(), inert_catalog],
    ] {
        assert!(matches!(
            TaskDefinitionCatalog::compose(&sources),
            Err(TaskCatalogError::InvalidCatalog("duplicate definition"))
        ));
    }
}

#[test]
fn typed_providers_reject_unassessed_registration_without_calling_assess() {
    for values in [vec![unassessed(1)], vec![definition(1), unassessed(2)]] {
        let probe = Rc::new(Probe::default());
        assert!(TaskCatalog::new(Rules {
            definitions: Rc::new(RefCell::new(values)),
            probe: probe.clone(),
        })
        .is_err());
        assert_eq!(probe.package_calls.get(), 1);
        assert_eq!(probe.definition_calls.get(), 1);
        assert_eq!(probe.assessments.get(), 0);
        assert_eq!(probe.drops.get(), 1);
    }
    let (typed, probe, _) = rules();
    assert_eq!(
        typed.assess(&definition(2).reference, "No fallback"),
        Err(TaskCatalogError::UnknownDefinition)
    );
    assert_eq!(probe.assessments.get(), 0);
    let bound = definition(1);
    let result = typed.assess(&bound.reference, "Typed evidence").unwrap();
    assert_eq!(result.definition, bound.reference);
    assert_eq!(result.policy, bound.assessment_policy.unwrap());
    assert_eq!(result.evidence_schema, bound.evidence_schema.unwrap());
    assert_eq!(
        result.outcome,
        AssessmentOutcome {
            verdict: AssessmentVerdict::Passed,
            feedback: vec!["Typed provider result".into()],
        }
    );
    assert_eq!(probe.assessments.get(), 1);
}

#[test]
fn actual_assessment_result_pins_remain_mandatory_and_non_optional() {
    let (typed, _, _) = rules();
    let result = typed
        .assess(&definition(1).reference, "Actual evidence")
        .unwrap();
    let raw = serde_json::to_value(&result).unwrap();
    assert_eq!(
        serde_json::from_value::<TaskAssessment>(raw.clone()).unwrap(),
        result
    );
    for field in ["policy", "evidence_schema"] {
        let mut omitted = raw.clone();
        omitted.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<TaskAssessment>(omitted).is_err());
        let mut null = raw.clone();
        null[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<TaskAssessment>(null).is_err());
    }
}
