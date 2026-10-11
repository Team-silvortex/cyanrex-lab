use super::*;

pub use content_fixture::{manifest, Fixture};

pub async fn unavailable_source() -> DurableAuthSource {
    input_fixture::unavailable_source().await
}

pub fn values() -> Vec<TaskDefinition> {
    let make = |package: &str, package_version: u64, bound: bool| {
        let pin = |name: String| VersionedName {
            name: name.parse().unwrap(),
            version: package_version.try_into().unwrap(),
        };
        TaskDefinition {
            schema_version: CoreSchemaVersion,
            reference: TaskDefinitionRef {
                package: pin(package.into()),
                name: format!("{package}.prepare").parse().unwrap(),
                version: 1.try_into().unwrap(),
            },
            title: format!("Definition {package} v{package_version}"),
            summary: "Frozen metadata; index never runs assessment".into(),
            evidence_schema: bound.then(|| pin(format!("{package}.text"))),
            assessment_policy: bound.then(|| pin(format!("{package}.review"))),
        }
    };
    vec![
        make("sample.document", 1, true),
        make("sample.document", 2, false),
        make("sample.memo", 1, false),
    ]
}

pub fn catalog(values: &[TaskDefinition]) -> TaskDefinitionCatalog {
    let packages: Vec<_> = values
        .iter()
        .map(|value| {
            TaskDefinitionCatalog::new(value.reference.package.clone(), vec![value.clone()])
                .unwrap()
        })
        .collect();
    TaskDefinitionCatalog::compose(&packages).unwrap()
}

pub fn index(f: &Fixture) -> SessionTaskContentIndexWorkspace {
    SessionTaskContentIndexWorkspace::new(&f.task_schema, scope()).unwrap()
}

pub fn configured(f: &Fixture, definitions: &[TaskDefinition]) -> SessionTaskContentIndexWorkspace {
    SessionTaskContentIndexWorkspace::from_definitions(
        &f.task_schema,
        scope(),
        &catalog(definitions),
    )
    .unwrap()
}

pub fn content(f: &Fixture, definitions: &[TaskDefinition]) -> SessionCatalogTaskContentWorkspace {
    SessionCatalogTaskContentWorkspace::from_definitions(
        &f.task_schema,
        scope(),
        f.artifact_workspace.clone(),
        &catalog(definitions),
    )
    .unwrap()
}

pub fn entry(snapshot: &TaskContentSnapshot) -> TaskContentIndexEntry {
    TaskContentIndexEntry {
        reference: snapshot.task.reference,
        revision: snapshot.task.revision,
        title: snapshot.task.title.clone(),
        status: snapshot.task.status,
        definition_ref: snapshot
            .task
            .definition
            .as_ref()
            .map(|value| value.reference.clone()),
        updated_at: snapshot.task.updated_at,
        payload_items: snapshot.manifest.payload().len(),
    }
}
