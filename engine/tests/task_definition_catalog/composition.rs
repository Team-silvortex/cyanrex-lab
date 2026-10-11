use super::*;

fn package_definition(package: &str, package_version: u64, task_version: u64) -> TaskDefinition {
    let mut value = definition(task_version);
    value.reference.package = pin(package, package_version);
    value.reference.name = format!("{package}.prepare").parse().unwrap();
    value.evidence_schema = Some(pin(&format!("{package}.text"), 1));
    value.assessment_policy = Some(pin(&format!("{package}.review"), task_version));
    value
}

fn registered(values: Vec<TaskDefinition>) -> TaskDefinitionCatalog {
    TaskDefinitionCatalog::new(values[0].reference.package.clone(), values).unwrap()
}

#[test]
fn composed_metadata_resolves_independent_packages_without_an_evaluator() {
    let editorial = package_definition("example.editorial", 3, 1);
    let translation = package_definition("example.translation", 2, 1);
    let sources = vec![
        registered(vec![editorial.clone()]),
        registered(vec![translation.clone()]),
    ];
    let composed = TaskDefinitionCatalog::compose(&sources).unwrap();
    assert_eq!(
        composed.definitions(),
        &[editorial.clone(), translation.clone()]
    );
    for value in [editorial, translation] {
        assert_eq!(composed.get(&value.reference), Some(&value));
        assert!(
            TaskDraft::from_definitions(&composed, &value.reference, "Private work", vec![])
                .is_ok()
        );
    }
}

#[test]
fn composition_keeps_every_reference_coordinate_and_source_order_exact() {
    let first = package_definition("example.editorial", 3, 2);
    let second = package_definition("example.editorial", 3, 1);
    let disjoint = package_definition("example.editorial", 3, 3);
    let next_package = package_definition("example.editorial", 4, 1);
    let translation = package_definition("example.translation", 3, 1);
    let sources = vec![
        registered(vec![first.clone(), second.clone()]),
        registered(vec![translation.clone()]),
        registered(vec![disjoint.clone()]),
        registered(vec![next_package.clone()]),
    ];
    let composed = TaskDefinitionCatalog::compose(&sources).unwrap();
    let expected = vec![first, second, translation, disjoint, next_package];
    assert_eq!(composed.definitions(), expected.as_slice());
    for value in expected {
        assert_eq!(composed.get(&value.reference), Some(&value));
        for field in 0..4 {
            let mut unknown = value.reference.clone();
            match field {
                0 => unknown.package.name = "example.unknown".parse().unwrap(),
                1 => unknown.package.version = 99.try_into().unwrap(),
                2 => unknown.name = "example.editorial.unknown".parse().unwrap(),
                3 => unknown.version = 99.try_into().unwrap(),
                _ => unreachable!(),
            }
            assert!(composed.get(&unknown).is_none());
            assert!(matches!(
                TaskDraft::from_definitions(&composed, &unknown, "Missing work", vec![]),
                Err(TaskStoreError::UnknownDefinition)
            ));
        }
    }
}

#[test]
fn nested_composition_flattens_without_sorting_or_dropping_definitions() {
    let first = package_definition("example.translation", 2, 2);
    let second = package_definition("example.editorial", 3, 1);
    let third = package_definition("example.translation", 2, 1);
    let last = package_definition("example.editorial", 4, 1);
    let nested = TaskDefinitionCatalog::compose(&[
        registered(vec![first.clone()]),
        registered(vec![second.clone()]),
        registered(vec![third.clone()]),
    ])
    .unwrap();
    let wrapped = TaskDefinitionCatalog::compose(std::slice::from_ref(&nested)).unwrap();
    assert_eq!(wrapped.definitions(), nested.definitions());
    let outer = TaskDefinitionCatalog::compose(&[wrapped, registered(vec![last.clone()])]).unwrap();
    assert_eq!(outer.definitions(), &[first, second, third, last]);
}

#[test]
fn composed_clones_share_frozen_owned_metadata_after_all_sources_are_dropped() {
    let original = package_definition("example.editorial", 3, 1);
    let other = package_definition("example.translation", 2, 1);
    let mut supplied = vec![original.clone()];
    let sources = vec![
        registered(supplied.clone()),
        registered(vec![other.clone()]),
    ];
    let composed = TaskDefinitionCatalog::compose(&sources).unwrap();
    supplied[0].title = "Changed outside the frozen catalogue".into();
    let copied = composed.clone();
    fn send_sync(_: &(impl Send + Sync)) {}
    send_sync(&copied);
    assert!(std::ptr::eq(
        composed.definitions().as_ptr(),
        copied.definitions().as_ptr()
    ));
    drop((sources, supplied, composed));
    assert_eq!(copied.definitions(), &[original.clone(), other.clone()]);
    assert_eq!(copied.get(&original.reference), Some(&original));
    assert_eq!(copied.get(&other.reference), Some(&other));
}

#[test]
fn composition_bounds_inputs_without_an_extra_distinct_package_limit() {
    assert!(matches!(
        TaskDefinitionCatalog::compose(&[]),
        Err(TaskCatalogError::InvalidCatalog(
            "expected 1 to 256 catalogs"
        ))
    ));
    let sources = (0..256)
        .map(|index| {
            registered(vec![package_definition(
                &format!("example.domain{index}"),
                1,
                1,
            )])
        })
        .collect::<Vec<_>>();
    let single = TaskDefinitionCatalog::compose(&sources[..1]).unwrap();
    assert_eq!(single.definitions(), sources[0].definitions());
    let composed = TaskDefinitionCatalog::compose(&sources).unwrap();
    assert_eq!(composed.definitions().len(), 256);
    for source in &sources {
        let value = &source.definitions()[0];
        assert_eq!(composed.get(&value.reference), Some(value));
    }
    let mut oversized = sources;
    oversized.push(registered(vec![package_definition("example.extra", 1, 1)]));
    assert!(matches!(
        TaskDefinitionCatalog::compose(&oversized),
        Err(TaskCatalogError::InvalidCatalog(
            "expected 1 to 256 catalogs"
        ))
    ));
}

#[test]
fn composition_limits_the_flattened_total_including_nested_sources() {
    let first = registered((1..=128).map(definition).collect());
    let second = registered((129..=256).map(definition).collect());
    let composed = TaskDefinitionCatalog::compose(&[first.clone(), second.clone()]).unwrap();
    let expected = (1..=256).map(definition).collect::<Vec<_>>();
    assert_eq!(composed.definitions(), expected.as_slice());
    let nested = TaskDefinitionCatalog::compose(&[first]).unwrap();
    let nested_limit = TaskDefinitionCatalog::compose(&[nested.clone(), second]).unwrap();
    assert_eq!(nested_limit.definitions(), expected.as_slice());
    let too_many = registered((129..=257).map(definition).collect());
    for sources in [
        vec![nested, too_many],
        vec![composed, registered(vec![definition(257)])],
    ] {
        assert!(matches!(
            TaskDefinitionCatalog::compose(&sources),
            Err(TaskCatalogError::InvalidCatalog(
                "expected 1 to 256 definitions"
            ))
        ));
    }
}

#[test]
fn duplicate_exact_references_never_deduplicate_override_or_choose_metadata() {
    let original = definition(1);
    let left = registered(vec![original.clone()]);
    let unrelated = registered(vec![package_definition("example.translation", 2, 1)]);
    let nested = TaskDefinitionCatalog::compose(&[left.clone(), unrelated]).unwrap();
    for field in 0..7 {
        let mut duplicate = original.clone();
        match field {
            0 => {}
            1 => duplicate.title = "Changed title".into(),
            2 => duplicate.summary = "Changed summary".into(),
            3 => {
                duplicate.evidence_schema.as_mut().unwrap().name =
                    "example.editorial.other".parse().unwrap()
            }
            4 => duplicate.evidence_schema.as_mut().unwrap().version = 2.try_into().unwrap(),
            5 => {
                duplicate.assessment_policy.as_mut().unwrap().name =
                    "example.editorial.other".parse().unwrap()
            }
            6 => duplicate.assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap(),
            _ => unreachable!(),
        }
        let right = registered(vec![duplicate]);
        for sources in [
            vec![left.clone(), right.clone()],
            vec![right.clone(), left.clone()],
            vec![nested.clone(), right],
        ] {
            assert!(matches!(
                TaskDefinitionCatalog::compose(&sources),
                Err(TaskCatalogError::InvalidCatalog("duplicate definition"))
            ));
        }
    }
    assert_eq!(left.definitions(), &[original]);
}

#[test]
fn composition_does_not_expand_single_package_registration_or_typed_dispatch() {
    let original = definition(1);
    let foreign = package_definition("example.translation", 2, 1);
    let other_version = package_definition("example.editorial", 4, 1);
    for extra in [foreign.clone(), other_version] {
        let values = vec![original.clone(), extra];
        assert!(matches!(
            TaskDefinitionCatalog::new(original.reference.package.clone(), values.clone()),
            Err(TaskCatalogError::InvalidCatalog(
                "definition belongs to another package"
            ))
        ));
        let probe = Rc::new(Probe::default());
        assert!(matches!(
            TaskCatalog::new(Rules {
                definitions: Rc::new(RefCell::new(values)),
                probe: probe.clone(),
            }),
            Err(TaskCatalogError::InvalidCatalog(
                "definition belongs to another package"
            ))
        ));
        assert_eq!(probe.package_calls.get(), 1);
        assert_eq!(probe.definition_calls.get(), 1);
        assert_eq!(probe.assessments.get(), 0);
        assert_eq!(probe.drops.get(), 1);
    }
    let (typed, probe, _) = rules();
    let composed = TaskDefinitionCatalog::compose(&[
        typed.definition_catalog().clone(),
        registered(vec![foreign.clone()]),
    ])
    .unwrap();
    assert_eq!(composed.get(&foreign.reference), Some(&foreign));
    assert!(typed.get(&foreign.reference).is_none());
    assert_eq!(
        typed.assess(&foreign.reference, "An unrelated provider's input"),
        Err(TaskCatalogError::UnknownDefinition)
    );
    assert_eq!(probe.assessments.get(), 0);
    let result = typed.assess(&original.reference, "Typed input").unwrap();
    assert_eq!(result.definition, original.reference);
    assert_eq!(result.policy, original.assessment_policy.unwrap());
    assert_eq!(result.evidence_schema, original.evidence_schema.unwrap());
    assert_eq!(probe.package_calls.get(), 1);
    assert_eq!(probe.definition_calls.get(), 1);
    assert_eq!(probe.assessments.get(), 1);
    drop(typed);
    assert_eq!(probe.drops.get(), 1);
    assert_eq!(composed.get(&foreign.reference), Some(&foreign));
}
