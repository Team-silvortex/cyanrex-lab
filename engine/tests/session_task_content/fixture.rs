use super::*;

pub fn manifest(title: &str, references: Vec<ArtifactRef>) -> TaskContentManifest {
    TaskContentManifest::new(
        title,
        references
            .into_iter()
            .map(|reference| {
                TextPayloadBinding::new(reference, "笔记😀.txt", "future.language").unwrap()
            })
            .collect(),
    )
    .unwrap()
}
pub struct Fixture {
    pub base: input_fixture::Fixture,
    pub tasks: TaskContentStore,
    pub workspace: SessionTaskContentWorkspace,
    pub task_schema: String,
    pub task_pool: PgPool,
}
impl std::ops::Deref for Fixture {
    type Target = input_fixture::Fixture;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl Fixture {
    pub async fn ready() -> Self {
        // Keep the independently prepared schema-2 fixture for explicit compatibility assertions.
        let base = input_fixture::Fixture::ready().await;
        let task_schema = format!("session_content_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {task_schema}"))
            .execute(&base.auth.base.admin)
            .await
            .unwrap();
        let task_pool = input_fixture::pool_for(&task_schema).await;
        let tasks = TaskContentStore::new(task_pool.clone(), scope());
        tasks.install_empty_namespace().await.unwrap();
        let workspace = SessionTaskContentWorkspace::new(
            &task_schema,
            scope(),
            base.artifact_workspace.clone(),
        )
        .unwrap();
        Self {
            base,
            tasks,
            workspace,
            task_schema,
            task_pool,
        }
    }
    pub async fn request(
        &self,
        token: &str,
        task: TaskId,
        value: TaskContentManifest,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        self.auth
            .source
            .create_session_content_task(token, &self.workspace, task, value)
            .await
    }
    pub async fn create(&self, refs: Vec<ArtifactRef>) -> TaskContentSnapshot {
        self.request(
            &self.auth.learner_token,
            id(),
            manifest("Private content task", refs),
        )
        .await
        .unwrap()
    }
    pub async fn get(
        &self,
        token: &str,
        task: TaskId,
    ) -> Result<Option<SessionTaskContent>, SessionTaskError> {
        self.auth
            .source
            .get_session_content_task(token, &self.workspace, task)
            .await
    }
    pub async fn replace(
        &self,
        token: &str,
        task: &TaskContentSnapshot,
        value: TaskContentManifest,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        self.auth
            .source
            .replace_session_content_task(
                token,
                &self.workspace,
                task.task.reference.task_id,
                task.task.revision,
                value,
            )
            .await
    }
    pub async fn transition(
        &self,
        token: &str,
        task: &TaskContentSnapshot,
        status: TaskStatus,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        self.auth
            .source
            .transition_session_content_task(
                token,
                &self.workspace,
                task.task.reference.task_id,
                task.task.revision,
                status,
            )
            .await
    }
    pub async fn unchanged(&self, task: &TaskContentSnapshot, events: i64) {
        assert_eq!(
            self.tasks
                .get(task.task.reference, task.task.owner)
                .await
                .unwrap(),
            Some(task.clone())
        );
        assert_eq!(self.count_task("collaboration_task_outbox").await, events);
    }
    pub async fn sql_task(&self, statement: &str) {
        query(statement).execute(&self.task_pool).await.unwrap();
    }
    pub async fn count_task(&self, table: &str) -> i64 {
        assert!(["collaboration_tasks", "collaboration_task_outbox"].contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.task_pool)
            .await
            .unwrap()
            .get("n")
    }
    pub async fn fault_trigger(&self, body: &str) {
        self.sql_task("CREATE SEQUENCE content_fault_calls").await;
        self.sql_task(&format!(
            "CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.content_fault_calls'); {body} RETURN NEW; END $$",
            self.task_schema
        ))
        .await;
        self.sql_task(
            "CREATE TRIGGER content_fault BEFORE INSERT ON collaboration_task_outbox
            FOR EACH ROW EXECUTE FUNCTION content_fault()",
        )
        .await;
    }
    pub async fn fault_calls(&self) -> i64 {
        let row = query("SELECT last_value, is_called FROM content_fault_calls")
            .fetch_one(&self.task_pool)
            .await
            .unwrap();
        assert!(
            row.get::<bool, _>("is_called"),
            "mutation did not reach the intended synthetic fault"
        );
        row.get("last_value")
    }
    pub async fn pause_task_event(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql_task(
            "CREATE FUNCTION pause_content_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 809); RETURN NEW; END $$",
        )
        .await;
        self.sql_task(
            "CREATE TRIGGER pause_content_event BEFORE INSERT ON collaboration_task_outbox
            FOR EACH ROW EXECUTE FUNCTION pause_content_event()",
        )
        .await;
        let mut holder = self.task_pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 809)")
            .bind(&self.task_schema)
            .execute(&mut *holder)
            .await
            .unwrap();
        holder
    }
    pub async fn cleanup(self) {
        self.task_pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.task_schema))
            .execute(&self.auth.base.admin)
            .await
            .unwrap();
        drop(self.workspace);
        drop(self.tasks);
        self.base.cleanup().await;
    }
}
pub async fn pair() -> (
    Fixture,
    TaskContentSnapshot,
    ArtifactRevision,
    ArtifactRevision,
) {
    let f = Fixture::ready().await;
    let old = f
        .create_artifact(&f.auth.learner_token, b"Original input")
        .await;
    let new = f
        .create_artifact(&f.auth.learner_token, b"Replacement input")
        .await;
    let task = f.create(vec![old.reference.clone()]).await;
    (f, task, old, new)
}
