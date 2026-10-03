use super::*;
use sha2::{Digest, Sha256};

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
pub fn kind() -> QualifiedName {
    "sample.document".parse().unwrap()
}
pub fn draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(kind(), "A document", "text/plain", bytes.to_vec()).unwrap()
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn private_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!("cyanrex-blob-test-{}", uuid::Uuid::new_v4()));
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    path
}
pub async fn pool_for(url: &str, schema: String, max: u32) -> PgPool {
    PgPoolOptions::new().max_connections(max).after_connect(move |connection, _| {
        let schema = schema.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                .bind(schema).execute(connection).await?;
            Ok(())
        })
    }).connect(url).await.unwrap()
}
pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: ArtifactStore,
    pub schema: String,
    pub directory: PathBuf,
}
impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable database required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("artifact_work_{}", uuid::Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, schema.clone(), 5).await;
        let directory = private_directory();
        let store = ArtifactStore::open(pool.clone(), scope(), &directory).unwrap();
        Self {
            admin,
            pool,
            store,
            schema,
            directory,
        }
    }
    pub async fn ready() -> Self {
        let f = Self::new().await;
        f.store.install_empty_namespace().await.unwrap();
        f
    }
    pub async fn sql(&self, sql: &str) {
        query(sql).execute(&self.pool).await.unwrap();
    }
    pub async fn count(&self, table: &str) -> i64 {
        assert!([
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox"
        ]
        .contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("n")
    }
    pub fn files(&self) -> usize {
        fs::read_dir(&self.directory).unwrap().count()
    }
    pub fn blob(&self) -> PathBuf {
        fs::read_dir(&self.directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
    pub async fn task_store(&self) -> (TaskStore, PgPool) {
        let schema = format!("{}_tasks", self.schema);
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&self.admin)
            .await
            .unwrap();
        let pool = pool_for(
            &std::env::var("CYANREX_TEST_DATABASE_URL").unwrap(),
            schema,
            2,
        )
        .await;
        let store = TaskStore::new(pool.clone(), scope());
        store.install_empty_namespace().await.unwrap();
        (store, pool)
    }
    pub async fn pause_events(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql(
            "CREATE FUNCTION pause_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 721); RETURN NEW; END $$",
        )
        .await;
        self.sql("CREATE TRIGGER pause_event BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION pause_event()").await;
        let mut blocker = self.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 721)")
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
                if n > 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.unwrap();
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        for schema in [&self.schema, &format!("{}_tasks", self.schema)] {
            query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
                .execute(&self.admin)
                .await
                .unwrap();
        }
        self.admin.close().await;
        fs::remove_dir_all(self.directory).unwrap();
    }
}
