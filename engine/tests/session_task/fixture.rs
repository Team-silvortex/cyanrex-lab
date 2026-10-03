use super::*;

pub struct Fixture {
    pub auth: identity_fixture::Fixture,
    pub pool: PgPool,
    pub tasks: TaskStore,
    pub workspace: SessionTaskWorkspace,
    pub schema: String,
}
impl Fixture {
    pub async fn ready() -> Self {
        let auth = identity_fixture::Fixture::ready().await;
        // Empty role presets + no deployment grant still allow the explicit private-task policy.
        let current = auth.current(auth.learner.binding.principal_id).await;
        let mut desired = current.policy;
        desired.membership.role_refs.clear();
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
        let schema = format!("task_session_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&auth.base.admin)
            .await
            .unwrap();
        let pool = pool_for(&schema).await;
        let tasks = TaskStore::new(pool.clone(), scope());
        tasks.install_empty_namespace().await.unwrap();
        let workspace = SessionTaskWorkspace::new(&schema, scope()).unwrap();
        Self {
            auth,
            pool,
            tasks,
            workspace,
            schema,
        }
    }
    pub async fn create(&self, token: &str) -> TaskSnapshot {
        self.auth
            .source
            .create_session_manual_task(token, &self.workspace, id(), "Write a document")
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
    pub async fn pause(&self) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql(
            "CREATE FUNCTION pause_task_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 741); RETURN NEW; END $$",
        )
        .await;
        self.sql("CREATE TRIGGER pause_task_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION pause_task_event()").await;
        let mut blocker = self.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 741)")
            .bind(&self.schema)
            .execute(&mut *blocker)
            .await
            .unwrap();
        blocker
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.auth.base.admin)
            .await
            .unwrap();
        self.auth.cleanup().await;
    }
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
