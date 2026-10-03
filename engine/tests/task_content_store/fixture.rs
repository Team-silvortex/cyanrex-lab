use super::*;

pub fn id<T: std::str::FromStr>() -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::new_v4().to_string().parse().unwrap()
}
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap(),
        workspace_id: "51f0325b-ea1b-42f9-9b39-71d063b8d327".parse().unwrap(),
    }
}
pub fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: "d7ed9514-330f-4cd7-9a30-32741ee8d0cf".parse().unwrap(),
    }
}
pub fn reference() -> TaskRef {
    TaskRef {
        workspace: scope(),
        task_id: "bdc46637-1a2a-4cd9-b617-915328788824".parse().unwrap(),
    }
}
pub fn rev(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}
pub fn artifact() -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
        revision_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}
pub fn content(title: &str, pins: Vec<ArtifactRef>) -> TaskContentManifest {
    TaskContentManifest::new(
        title,
        pins.into_iter()
            .map(|pin| TextPayloadBinding::new(pin, "笔记😀.md", "markdown").unwrap())
            .collect(),
    )
    .unwrap()
}
pub fn manifest() -> TaskContentManifest {
    content("A non-teaching document", vec![artifact()])
}

pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: TaskContentStore,
    pub schema: String,
}
impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable database required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("task_content_{}", uuid::Uuid::new_v4().simple());
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
        Self {
            store: TaskContentStore::new(pool.clone(), scope()),
            admin,
            pool,
            schema,
        }
    }
    pub async fn ready() -> Self {
        let f = Self::new().await;
        f.store.install_empty_namespace().await.unwrap();
        f
    }
    pub async fn create(&self) -> TaskContentSnapshot {
        self.store
            .create(reference().task_id, owner(), manifest())
            .await
            .unwrap()
    }
    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }
    pub async fn count(&self, table: &str) -> i64 {
        assert!(["collaboration_tasks", "collaboration_task_outbox"].contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("n")
    }
    pub async fn unchanged(&self, expected: &TaskContentSnapshot, events: i64) {
        assert_eq!(
            self.store
                .get(expected.task.reference, expected.task.owner)
                .await
                .unwrap(),
            Some(expected.clone())
        );
        assert_eq!(self.count("collaboration_task_outbox").await, events);
    }
    pub async fn event_types(&self) -> Vec<String> {
        query("SELECT event_type FROM collaboration_task_outbox ORDER BY revision")
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get("event_type"))
            .collect()
    }
    pub async fn recorded(&self, expected: &TaskContentSnapshot) {
        for table in ["collaboration_tasks", "collaboration_task_outbox"] {
            let row = query(&format!("SELECT snapshot, manifest FROM {table} WHERE task_id = $1 ORDER BY revision DESC LIMIT 1"))
                .bind(expected.task.reference.task_id.as_uuid()).fetch_one(&self.pool).await.unwrap();
            let raw: String = row.get("snapshot");
            let raw_manifest: String = row.get("manifest");
            assert_eq!(
                serde_json::from_str::<TaskSnapshot>(&raw).unwrap(),
                expected.task
            );
            assert_eq!(
                TaskContentManifest::parse_json(raw_manifest.as_bytes()).unwrap(),
                expected.manifest
            );
        }
    }
    pub async fn pause_events(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql(
            "CREATE FUNCTION pause_content_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 797); RETURN NEW; END $$",
        )
        .await;
        self.sql("CREATE TRIGGER pause_content_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION pause_content_event()").await;
        let mut blocker = self.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 797)")
            .bind(&self.schema)
            .execute(&mut *blocker)
            .await
            .unwrap();
        blocker
    }
    pub async fn wait_blocked(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let n: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock'")
                    .bind(&self.schema).fetch_one(&self.admin).await.unwrap().get("n");
                if n > 0 { return; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("content operation did not reach the synthetic lock");
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
