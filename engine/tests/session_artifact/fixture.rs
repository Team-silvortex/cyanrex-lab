use super::*;

pub fn draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(
        "sample.document".parse().unwrap(),
        "A private document",
        "text/plain",
        bytes.to_vec(),
    )
    .unwrap()
}
pub fn private_directory() -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let root = std::env::temp_dir().join(format!("cyanrex-session-artifact-{}", Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    root
}
pub async fn pool_for(schema: &str) -> PgPool {
    let schema = schema.to_string();
    PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false)")
                    .bind(schema)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
}
pub struct Fixture {
    pub auth: identity_fixture::Fixture,
    pub schema: String,
    pub pool: PgPool,
    pub artifacts: ArtifactStore,
    pub workspace: SessionArtifactWorkspace,
    pub directory: PathBuf,
}
impl Fixture {
    pub async fn ready() -> Self {
        let auth = identity_fixture::Fixture::ready().await;
        let current = auth.current(auth.learner.binding.principal_id).await;
        let mut desired = current.policy;
        desired.membership.role_refs.clear();
        assert!(!desired.deployment_granted);
        auth.source
            .apply_session_policy_command(
                &auth.manager_token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: Some(current.revision),
                    desired,
                },
            )
            .await
            .unwrap();
        let schema = format!("artifact_session_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&auth.base.admin)
            .await
            .unwrap();
        let pool = pool_for(&schema).await;
        let directory = private_directory();
        let artifacts = ArtifactStore::open(pool.clone(), scope(), &directory).unwrap();
        artifacts.install_empty_namespace().await.unwrap();
        let workspace = auth
            .source
            .open_artifact_workspace(&schema, scope(), &directory)
            .unwrap();
        Self {
            auth,
            schema,
            pool,
            artifacts,
            workspace,
            directory,
        }
    }
    pub async fn create(&self, token: &str, bytes: &[u8]) -> ArtifactRevision {
        self.auth
            .source
            .create_session_artifact(token, &self.workspace, id(), id(), draft(bytes))
            .await
            .unwrap()
    }
    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
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
    pub async fn pause(&self) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql(
            "CREATE FUNCTION pause_artifact_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 751); RETURN NEW; END $$",
        )
        .await;
        self.sql("CREATE TRIGGER pause_artifact_event BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION pause_artifact_event()").await;
        let mut blocker = self.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 751)")
            .bind(&self.schema)
            .execute(&mut *blocker)
            .await
            .unwrap();
        blocker
    }
    pub async fn assert_pool_restored(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut connections = Vec::new();
            for _ in 0..4 {
                let mut connection = self.auth.base.pool.acquire().await.unwrap();
                let row = query("SELECT current_schema()::text AS namespace, current_setting('search_path') AS path")
                    .fetch_one(&mut *connection).await.unwrap();
                assert_eq!(row.get::<String,_>("namespace"), self.auth.base.schema);
                assert_eq!(row.get::<String,_>("path"), self.auth.base.schema);
                connections.push(connection);
            }
        }).await.unwrap();
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.auth.base.admin)
            .await
            .unwrap();
        self.auth.cleanup().await;
        drop(self.workspace);
        drop(self.artifacts);
        fs::remove_dir_all(self.directory).unwrap();
    }
}
