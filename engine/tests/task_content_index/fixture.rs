use super::*;
use cyanrex_engine::services::{
    task_catalog::TaskDefinitionCatalog,
    task_store::{TaskContentIndexEntry, TaskContentSnapshot, TaskContentStore},
};
use sqlx_core::{query::query, row::Row};

pub(super) fn rev(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}

pub(super) fn task(value: u64) -> TaskRef {
    TaskRef {
        workspace: scope(),
        task_id: format!("00000000-0000-0000-0000-{value:012x}")
            .parse()
            .unwrap(),
    }
}

pub(super) fn other_owner() -> PrincipalRef {
    PrincipalRef {
        principal_id: "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap(),
        ..owner()
    }
}

pub(super) fn definitions() -> (TaskDefinitionCatalog, Vec<TaskDefinition>) {
    let pin = |name: &str, version| VersionedName {
        name: name.parse().unwrap(),
        version: rev(version),
    };
    let values: Vec<_> = [
        ("sample.documents", 1, 1, true),
        ("sample.documents", 2, 3, false),
        ("sample.notes", 1, 4, true),
    ]
    .into_iter()
    .map(
        |(package, package_version, version, bound)| TaskDefinition {
            schema_version: CoreSchemaVersion,
            reference: TaskDefinitionRef {
                package: pin(package, package_version),
                name: format!("{package}.prepare").parse().unwrap(),
                version: rev(version),
            },
            title: "Prepare non-teaching work".into(),
            summary: "Frozen metadata only".into(),
            evidence_schema: bound.then(|| pin(&format!("{package}.text"), package_version)),
            assessment_policy: bound.then(|| pin(&format!("{package}.editorial"), package_version)),
        },
    )
    .collect();
    let catalogs: Vec<_> = values
        .iter()
        .map(|definition| {
            TaskDefinitionCatalog::new(
                definition.reference.package.clone(),
                vec![definition.clone()],
            )
            .unwrap()
        })
        .collect();
    (TaskDefinitionCatalog::compose(&catalogs).unwrap(), values)
}

pub(super) fn manifest(value: u64) -> TaskContentManifest {
    // Deliberately no Artifact namespace or file is installed. These are metadata pins only.
    let pins = (0..value % 3)
        .map(|index| {
            let reference = ArtifactRef {
                workspace: scope(),
                artifact_id: format!("10000000-0000-0000-0000-{:012x}", value + 1)
                    .parse()
                    .unwrap(),
                revision_id: format!("20000000-0000-0000-0000-{:012x}", index + 1)
                    .parse()
                    .unwrap(),
                sha256: "a".repeat(64).parse().unwrap(),
            };
            TextPayloadBinding::new(reference, "笔记😀.txt", "future.language").unwrap()
        })
        .collect();
    TaskContentManifest::new(&format!("Task {value} 📝"), pins).unwrap()
}

pub(super) fn projected(snapshot: &TaskContentSnapshot) -> TaskContentIndexEntry {
    TaskContentIndexEntry {
        reference: snapshot.task.reference,
        revision: snapshot.task.revision,
        title: snapshot.task.title.clone(),
        status: snapshot.task.status,
        definition_ref: snapshot
            .task
            .definition
            .as_ref()
            .map(|definition| definition.reference.clone()),
        updated_at: snapshot.task.updated_at,
        payload_items: snapshot.manifest.payload().len(),
    }
}

pub(super) struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub schema: String,
    pub manual: TaskContentStore,
    pub catalog: TaskContentStore,
    pub index: TaskContentIndexStore,
    pub definitions: Vec<TaskDefinition>,
}

impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable database required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("task_content_index_{}", uuid::Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let selected = schema.clone();
        let pool = PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
            let selected = selected.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                    .bind(selected).execute(connection).await?;
                Ok(())
            })
        }).connect(&url).await.unwrap();
        let (catalog, definitions) = definitions();
        Self {
            manual: TaskContentStore::new(pool.clone(), scope()),
            catalog: TaskContentStore::from_definitions(pool.clone(), scope(), &catalog),
            index: TaskContentIndexStore::from_definitions(pool.clone(), scope(), &catalog),
            admin,
            pool,
            schema,
            definitions,
        }
    }

    pub async fn ready() -> Self {
        let f = Self::new().await;
        f.manual.install_empty_namespace().await.unwrap();
        f
    }

    pub async fn create(
        &self,
        value: u64,
        actor: PrincipalRef,
        definition: Option<usize>,
    ) -> TaskContentSnapshot {
        match definition {
            Some(index) => self
                .catalog
                .create_catalog(
                    task(value).task_id,
                    actor,
                    &self.definitions[index].reference,
                    manifest(value),
                )
                .await
                .unwrap(),
            None => self
                .manual
                .create(task(value).task_id, actor, manifest(value))
                .await
                .unwrap(),
        }
    }

    pub async fn rows(&self) -> Vec<Vec<String>> {
        let mut result = Vec::new();
        for table in [
            "collaboration_task_schema",
            "collaboration_tasks",
            "collaboration_task_outbox",
        ] {
            let mut rows: Vec<String> = query(&format!(
                "SELECT row_to_json(t)::text AS value FROM {table} t"
            ))
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("value"))
            .collect();
            rows.sort();
            result.push(rows);
        }
        result
    }

    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }

    pub async fn set_snapshot(&self, reference: TaskRef, snapshot: &TaskSnapshot) {
        let raw = serde_json::to_string(snapshot).unwrap();
        self.set_column(reference, "snapshot", &raw).await;
    }

    pub async fn set_column(&self, reference: TaskRef, column: &str, raw: &str) {
        assert!(["snapshot", "manifest"].contains(&column));
        for table in ["collaboration_tasks", "collaboration_task_outbox"] {
            assert_eq!(
                query(&format!(
                    "UPDATE {table} SET {column} = $1 WHERE task_id = $2"
                ))
                .bind(raw)
                .bind(reference.task_id.as_uuid())
                .execute(&self.pool)
                .await
                .unwrap()
                .rows_affected(),
                1
            );
        }
    }

    pub async fn restore(&self, snapshot: &TaskContentSnapshot) {
        self.set_snapshot(snapshot.task.reference, &snapshot.task)
            .await;
        self.set_column(
            snapshot.task.reference,
            "manifest",
            &serde_json::to_string(&snapshot.manifest).unwrap(),
        )
        .await;
    }

    pub async fn reject_unchanged(&self, after: Option<TaskRef>, limit: Option<usize>) {
        let before = self.rows().await;
        assert_eq!(
            self.index.list(owner(), after, limit).await,
            Err(TaskStoreError::InvalidRecord)
        );
        assert_eq!(self.rows().await, before);
    }

    pub async fn wait_blocked(&self, blocker: i32) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let n: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name = $1 AND $2 = ANY(pg_blocking_pids(pid))")
                    .bind(&self.schema).bind(blocker).fetch_one(&self.admin).await.unwrap().get("n");
                if n > 0 { return; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("index did not reach the intended table lock");
    }

    pub async fn assert_pool_restored(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut connection = self.pool.acquire().await.unwrap();
                let selected: String = query("SELECT current_schema() AS selected")
                    .fetch_one(&mut *connection)
                    .await
                    .unwrap()
                    .get("selected");
                assert_eq!(selected, self.schema);
                held.push(connection);
            }
        })
        .await
        .unwrap();
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
