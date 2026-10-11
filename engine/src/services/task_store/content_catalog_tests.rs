use super::*;

fn versioned(name: &str) -> VersionedName {
    VersionedName {
        name: name.parse().unwrap(),
        version: 1.try_into().unwrap(),
    }
}

fn definition() -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: versioned("example.documents"),
            name: "example.documents.summary".parse().unwrap(),
            version: 1.try_into().unwrap(),
        },
        title: "A document task".into(),
        summary: "Frozen metadata, no evaluator".into(),
        evidence_schema: Some(versioned("example.documents.text")),
        assessment_policy: Some(versioned("example.documents.words")),
    }
}

fn catalog() -> TaskDefinitionCatalog {
    let definition = definition();
    TaskDefinitionCatalog::new(definition.reference.package.clone(), vec![definition]).unwrap()
}

async fn closed_pool() -> PgPool {
    let pool = sqlx::PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}

#[tokio::test]
async fn catalog_content_records_require_some_and_the_complete_frozen_definition() {
    let pool = closed_pool().await;
    let manual = TaskContentStore::new(pool.clone(), tests::snapshot().task.reference.workspace);
    let store = TaskContentStore::from_definitions(
        pool,
        tests::snapshot().task.reference.workspace,
        &catalog(),
    );
    let mut original = tests::snapshot();
    assert_eq!(manual.validate_record(&original), Ok(()));
    assert_eq!(
        store.validate_record(&original),
        Err(TaskStoreError::InvalidRecord)
    );
    original.task.definition = Some(definition());
    assert_eq!(store.validate_record(&original), Ok(()));
    assert_eq!(
        manual.validate_record(&original),
        Err(TaskStoreError::InvalidRecord)
    );
    let mut drift = vec![definition(); 6];
    drift[0].title.push('!');
    drift[1].summary.push('!');
    drift[2].evidence_schema.as_mut().unwrap().name =
        "example.documents.other_text".parse().unwrap();
    drift[3].evidence_schema.as_mut().unwrap().version = 2.try_into().unwrap();
    drift[4].assessment_policy.as_mut().unwrap().name =
        "example.documents.other_words".parse().unwrap();
    drift[5].assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap();
    for definition in drift {
        let mut changed = original.clone();
        changed.task.definition = Some(definition);
        assert_eq!(
            store.validate_record(&changed),
            Err(TaskStoreError::InvalidRecord)
        );
    }
    let mut unknown = original.clone();
    unknown.task.definition.as_mut().unwrap().reference.version = 2.try_into().unwrap();
    assert_eq!(
        store.validate_record(&unknown),
        Err(TaskStoreError::InvalidRecord)
    );
}

#[tokio::test]
async fn catalog_content_replacement_and_transition_preserve_exact_definition_identity() {
    let metadata = catalog();
    let mut original = tests::snapshot();
    original.task.definition = Some(definition());
    let store = TaskContentStore::from_definitions(
        closed_pool().await,
        original.task.reference.workspace,
        &metadata,
    );
    let before = original.clone();
    assert_eq!(store.validate_record(&original), Ok(()));
    let manifest = TaskContentManifest::new("Renamed", vec![]).unwrap();
    let next = prepare_replacement(
        &original,
        original.task.revision,
        manifest.clone(),
        tests::now(),
    )
    .unwrap();
    assert_eq!(store.validate_record(&next), Ok(()));
    assert_eq!(next.task.definition, original.task.definition);
    assert_eq!(next.manifest, manifest);
    let transitioned =
        prepare_transition(&next, next.task.revision, TaskStatus::Ready, tests::now()).unwrap();
    assert_eq!(store.validate_record(&transitioned), Ok(()));
    assert_eq!(transitioned.task.definition, original.task.definition);
    assert_eq!(transitioned.manifest, next.manifest);
    assert_eq!(original, before);
}
