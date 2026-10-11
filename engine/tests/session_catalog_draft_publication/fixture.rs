use super::*;
use std::os::unix::fs::DirBuilderExt;

pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";

pub fn definition() -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: VersionedName {
                name: "sample.document".parse().unwrap(),
                version: 2.try_into().unwrap(),
            },
            name: "sample.document.prepare".parse().unwrap(),
            version: 3.try_into().unwrap(),
        },
        title: "Frozen document task".into(),
        summary: "No automatic assessment is still catalogue metadata".into(),
        evidence_schema: None,
        assessment_policy: None,
    }
}

pub fn plan(texts: &[&str]) -> TaskDraftPublicationPlan {
    let payload: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            serde_json::json!({"id":format!("local_{index}"),"revision":1,"kind":"text",
                "filename":format!("item-{index}.txt"),"language":"future.language","text":text})
        })
        .collect();
    TaskDraftPublicationPlan::parse_json(
        &serde_json::to_vec(
            &serde_json::json!({"format":"cyanrex.task-draft","version":1,
            "title":"Catalogue draft 中文😀","payload":payload}),
        )
        .unwrap(),
    )
    .unwrap()
}

pub struct Offline {
    pub source: DurableAuthSource,
    pub workspace: SessionCatalogTaskContentWorkspace,
    pub root: PathBuf,
}
impl Offline {
    pub async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let scope = WorkspaceRef {
            authority_id: Uuid::new_v4().try_into().unwrap(),
            workspace_id: Uuid::new_v4().try_into().unwrap(),
        };
        let source = DurableAuthSource::new(pool, scope.authority_id);
        let root = std::env::temp_dir().join(format!("catalog-draft-{}", Uuid::new_v4().simple()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let artifacts = source
            .open_artifact_workspace("private_artifacts", scope, &root)
            .unwrap();
        let value = definition();
        let catalog =
            TaskDefinitionCatalog::new(value.reference.package.clone(), vec![value]).unwrap();
        let workspace = SessionCatalogTaskContentWorkspace::from_definitions(
            "private_tasks",
            scope,
            artifacts,
            &catalog,
        )
        .unwrap();
        Self {
            source,
            workspace,
            root,
        }
    }
}
impl Drop for Offline {
    fn drop(&mut self) {
        assert_eq!(fs::read_dir(&self.root).unwrap().count(), 0);
        fs::remove_dir(&self.root).unwrap();
    }
}
