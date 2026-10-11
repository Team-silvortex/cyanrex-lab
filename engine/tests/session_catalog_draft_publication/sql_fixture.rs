use super::*;

pub use manual_fixture::{
    artifact_fault_calls, backend, blocked_by, counts, pause_artifact_event, uncertain_artifact,
    uncertain_task, wait_expired, Fixture,
};

pub fn values() -> Vec<TaskDefinition> {
    let unbound = definition();
    let mut bound = unbound.clone();
    bound.reference.package.version = 1.try_into().unwrap();
    bound.reference.version = 1.try_into().unwrap();
    bound.evidence_schema = Some(VersionedName {
        name: "sample.document.text".parse().unwrap(),
        version: 1.try_into().unwrap(),
    });
    bound.assessment_policy = Some(VersionedName {
        name: "sample.document.editorial".parse().unwrap(),
        version: 1.try_into().unwrap(),
    });
    let mut memo = unbound.clone();
    memo.reference.package.name = "sample.memo".parse().unwrap();
    memo.reference.name = "sample.memo.prepare".parse().unwrap();
    vec![bound, unbound, memo]
}

pub fn catalog(values: &[TaskDefinition]) -> TaskDefinitionCatalog {
    let catalogs: Vec<_> = values
        .iter()
        .map(|value| {
            TaskDefinitionCatalog::new(value.reference.package.clone(), vec![value.clone()])
                .unwrap()
        })
        .collect();
    TaskDefinitionCatalog::compose(&catalogs).unwrap()
}

pub fn workspace(f: &Fixture) -> SessionCatalogTaskContentWorkspace {
    SessionCatalogTaskContentWorkspace::from_definitions(
        &f.task_schema,
        scope(),
        f.artifact_workspace.clone(),
        &catalog(&values()),
    )
    .unwrap()
}

pub fn prepare(
    f: &Fixture,
    definition: &TaskDefinition,
    items: &[(&str, &str, &str)],
) -> SessionCatalogTaskDraftPublication {
    f.auth
        .source
        .prepare_catalog_task_draft_publication(
            &f.auth.learner_token,
            workspace(f),
            &definition.reference,
            manual_fixture::plan(items),
        )
        .unwrap()
}

pub async fn read(f: &Fixture, attempt: &SessionCatalogTaskDraftPublication) -> SessionTaskContent {
    f.auth
        .source
        .get_session_catalog_content_task(
            &f.auth.learner_token,
            &workspace(f),
            attempt.task_ref().task_id,
        )
        .await
        .unwrap()
        .unwrap()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Observed {
    pub rows: Vec<(String, Vec<String>)>,
    pub files: Vec<(String, Vec<u8>)>,
}
pub async fn observed(f: &Fixture) -> Observed {
    let mut rows = Vec::new();
    for (pool, tables) in [
        (
            &f.task_pool,
            vec![
                "collaboration_task_schema",
                "collaboration_tasks",
                "collaboration_task_outbox",
            ],
        ),
        (
            &f.artifact_pool,
            vec![
                "collaboration_artifact_schema",
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
                "collaboration_artifact_outbox",
            ],
        ),
    ] {
        for table in tables {
            let mut values: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {table} t"
            ))
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("value"))
            .collect();
            values.sort();
            rows.push((table.into(), values));
        }
    }
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    Observed { rows, files }
}
