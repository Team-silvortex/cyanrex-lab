use super::*;

pub async fn unavailable_source() -> DurableAuthSource {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    DurableAuthSource::new(pool, authority())
}
pub fn offline_workspace(
    source: &DurableAuthSource,
    root: &std::path::Path,
) -> SessionTaskInputsWorkspace {
    SessionTaskInputsWorkspace::new(
        SessionTaskWorkspace::new("private_tasks", scope()).unwrap(),
        source
            .open_artifact_workspace("private_artifacts", scope(), root)
            .unwrap(),
    )
    .unwrap()
}

pub fn draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(
        "sample.document".parse().unwrap(),
        "An input document",
        "text/plain",
        bytes.to_vec(),
    )
    .unwrap()
}
pub fn private_directory() -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let root = std::env::temp_dir().join(format!("cyanrex-session-inputs-{}", Uuid::new_v4()));
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
    pub tasks: TaskStore,
    pub artifacts: ArtifactStore,
    pub workspace: SessionTaskInputsWorkspace,
    pub task_workspace: SessionTaskWorkspace,
    pub artifact_workspace: SessionArtifactWorkspace,
    pub task_schema: String,
    pub artifact_schema: String,
    pub task_pool: PgPool,
    pub artifact_pool: PgPool,
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
        let task_schema = format!("inputs_tasks_{}", Uuid::new_v4().simple());
        let artifact_schema = format!("inputs_artifacts_{}", Uuid::new_v4().simple());
        for schema in [&task_schema, &artifact_schema] {
            query(&format!("CREATE SCHEMA {schema}"))
                .execute(&auth.base.admin)
                .await
                .unwrap();
        }
        let task_pool = pool_for(&task_schema).await;
        let artifact_pool = pool_for(&artifact_schema).await;
        let directory = private_directory();
        let tasks = TaskStore::new(task_pool.clone(), scope());
        tasks.install_empty_namespace().await.unwrap();
        let artifacts = ArtifactStore::open(artifact_pool.clone(), scope(), &directory).unwrap();
        artifacts.install_empty_namespace().await.unwrap();
        let task_workspace = SessionTaskWorkspace::new(&task_schema, scope()).unwrap();
        let artifact_workspace = auth
            .source
            .open_artifact_workspace(&artifact_schema, scope(), &directory)
            .unwrap();
        let workspace =
            SessionTaskInputsWorkspace::new(task_workspace.clone(), artifact_workspace.clone())
                .unwrap();
        Self {
            auth,
            tasks,
            artifacts,
            workspace,
            task_workspace,
            artifact_workspace,
            task_schema,
            artifact_schema,
            task_pool,
            artifact_pool,
            directory,
        }
    }
    pub async fn create_artifact(&self, token: &str, bytes: &[u8]) -> ArtifactRevision {
        self.auth
            .source
            .create_session_artifact(token, &self.artifact_workspace, id(), id(), draft(bytes))
            .await
            .unwrap()
    }
    pub async fn create(&self, token: &str, inputs: Vec<ArtifactRef>) -> TaskSnapshot {
        self.auth
            .source
            .create_session_input_task(
                token,
                &self.workspace,
                id(),
                "Review input documents",
                inputs,
            )
            .await
            .unwrap()
    }
    pub async fn request(
        &self,
        token: &str,
        task: TaskId,
        inputs: Vec<ArtifactRef>,
    ) -> Result<TaskSnapshot, SessionTaskError> {
        self.auth
            .source
            .create_session_input_task(token, &self.workspace, task, "Input task", inputs)
            .await
    }
    pub async fn get(
        &self,
        token: &str,
        task: TaskId,
    ) -> Result<Option<SessionTaskInputs>, SessionTaskError> {
        self.auth
            .source
            .get_session_input_task(token, &self.workspace, task)
            .await
    }
    pub async fn transition(
        &self,
        token: &str,
        task: &TaskSnapshot,
        status: TaskStatus,
    ) -> Result<TaskSnapshot, SessionTaskError> {
        self.auth
            .source
            .transition_session_input_task(
                token,
                &self.workspace,
                task.reference.task_id,
                task.revision,
                status,
            )
            .await
    }
    pub async fn replace_retired_learner(&self) -> (String, PrincipalRef) {
        let username = &self.auth.learner.binding.username;
        query("DELETE FROM users WHERE username = $1")
            .bind(username.as_str())
            .execute(&self.auth.base.pool)
            .await
            .unwrap();
        let account = self.auth.source.register(username, PASSWORD).await.unwrap();
        let token = self
            .auth
            .source
            .login(username, PASSWORD, &otp(&account.bootstrap.secret))
            .await
            .unwrap()
            .token;
        let bound = self
            .auth
            .source
            .bind_session_account(
                &self.auth.manager_token,
                &SessionBindCommand {
                    command_id: id(),
                    workspace: scope(),
                    target: account.account,
                },
            )
            .await
            .unwrap()
            .entry
            .after;
        self.auth
            .source
            .apply_session_policy_command(
                &self.auth.manager_token,
                &SessionPolicyCommand {
                    command_id: id(),
                    expected_revision: None,
                    desired: LegacyAccessPolicy {
                        membership: Membership {
                            workspace: scope(),
                            principal_id: bound.binding.principal_id,
                            role_refs: vec![],
                            status: MembershipStatus::Active,
                        },
                        deployment_granted: false,
                    },
                },
            )
            .await
            .unwrap();
        (token, bound.principal.reference)
    }
    pub async fn sql_task(&self, statement: &str) {
        query(statement).execute(&self.task_pool).await.unwrap();
    }
    pub async fn sql_artifact(&self, statement: &str) {
        query(statement).execute(&self.artifact_pool).await.unwrap();
    }
    pub async fn count_task(&self, table: &str) -> i64 {
        assert!(["collaboration_tasks", "collaboration_task_outbox"].contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.task_pool)
            .await
            .unwrap()
            .get("n")
    }
    pub async fn count_artifact(&self, table: &str) -> i64 {
        assert!([
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox"
        ]
        .contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.artifact_pool)
            .await
            .unwrap()
            .get("n")
    }
    pub fn files(&self) -> usize {
        fs::read_dir(&self.directory).unwrap().count()
    }
    pub fn blob(&self, reference: &ArtifactRef) -> PathBuf {
        self.directory.join(format!(
            "{}-{}-{}-{}-{}",
            reference.workspace.authority_id,
            reference.workspace.workspace_id,
            reference.artifact_id,
            reference.revision_id,
            reference.sha256
        ))
    }
    pub async fn pause_task_event(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql_task(
            "CREATE FUNCTION pause_input_task_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 761); RETURN NEW; END $$",
        )
        .await;
        self.sql_task("CREATE TRIGGER pause_input_task_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION pause_input_task_event()").await;
        let mut blocker = self.task_pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 761)")
            .bind(&self.task_schema)
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
        self.task_pool.close().await;
        self.artifact_pool.close().await;
        for schema in [&self.task_schema, &self.artifact_schema] {
            query(&format!("DROP SCHEMA {schema} CASCADE"))
                .execute(&self.auth.base.admin)
                .await
                .unwrap();
        }
        self.auth.cleanup().await;
        drop(self.workspace);
        drop(self.artifact_workspace);
        drop(self.artifacts);
        fs::remove_dir_all(self.directory).unwrap();
    }
}
