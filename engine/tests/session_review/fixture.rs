use super::*;

pub fn policy() -> VersionedName {
    VersionedName {
        name: "sample.document.editorial".parse().unwrap(),
        version: 1.try_into().unwrap(),
    }
}
pub fn edit(verdict: HumanReviewVerdict, comment: &str) -> HumanReviewEdit {
    HumanReviewEdit::new(verdict, comment).unwrap()
}
pub fn artifact_draft(bytes: &[u8]) -> ArtifactDraft {
    ArtifactDraft::new(
        "sample.document".parse().unwrap(),
        "Review document",
        "text/plain",
        bytes.to_vec(),
    )
    .unwrap()
}
pub fn synthetic_ref() -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}
pub fn assessment() -> TaskAssessment {
    TaskAssessment {
        definition: TaskDefinitionRef {
            package: VersionedName {
                name: "sample.document".parse().unwrap(),
                version: 1.try_into().unwrap(),
            },
            name: "sample.document.spelling".parse().unwrap(),
            version: 1.try_into().unwrap(),
        },
        policy: policy(),
        evidence_schema: VersionedName {
            name: "sample.document.text".parse().unwrap(),
            version: 1.try_into().unwrap(),
        },
        outcome: AssessmentOutcome {
            verdict: AssessmentVerdict::Passed,
            feedback: vec!["Spelling checked".into()],
        },
    }
}
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
) -> SessionReviewWorkspace {
    SessionReviewWorkspace::new(
        "private_reviews",
        scope(),
        source
            .open_artifact_workspace("private_artifacts", scope(), root)
            .unwrap(),
        policy(),
    )
    .unwrap()
}
pub fn private_directory() -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let root = std::env::temp_dir().join(format!("cyanrex-session-review-{}", Uuid::new_v4()));
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
    pub reviews: ReviewStore,
    pub review_pool: PgPool,
    pub review_schema: String,
    pub artifacts: ArtifactStore,
    pub artifact_pool: PgPool,
    pub artifact_schema: String,
    pub artifact_workspace: SessionArtifactWorkspace,
    pub workspace: SessionReviewWorkspace,
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
        let review_schema = format!("reviews_session_{}", Uuid::new_v4().simple());
        let artifact_schema = format!("review_artifacts_{}", Uuid::new_v4().simple());
        for schema in [&review_schema, &artifact_schema] {
            query(&format!("CREATE SCHEMA {schema}"))
                .execute(&auth.base.admin)
                .await
                .unwrap();
        }
        let review_pool = pool_for(&review_schema).await;
        let artifact_pool = pool_for(&artifact_schema).await;
        let directory = private_directory();
        let reviews = ReviewStore::new(review_pool.clone(), scope());
        reviews.install_empty_namespace().await.unwrap();
        let artifacts = ArtifactStore::open(artifact_pool.clone(), scope(), &directory).unwrap();
        artifacts.install_empty_namespace().await.unwrap();
        let artifact_workspace = auth
            .source
            .open_artifact_workspace(&artifact_schema, scope(), &directory)
            .unwrap();
        let workspace = SessionReviewWorkspace::new(
            &review_schema,
            scope(),
            artifact_workspace.clone(),
            policy(),
        )
        .unwrap();
        Self {
            auth,
            reviews,
            review_pool,
            review_schema,
            artifacts,
            artifact_pool,
            artifact_schema,
            artifact_workspace,
            workspace,
            directory,
        }
    }
    pub async fn create_artifact(&self, token: &str, bytes: &[u8]) -> ArtifactRevision {
        self.auth
            .source
            .create_session_artifact(
                token,
                &self.artifact_workspace,
                id(),
                id(),
                artifact_draft(bytes),
            )
            .await
            .unwrap()
    }
    pub async fn create(
        &self,
        token: &str,
        targets: Vec<ArtifactRef>,
        evidence: Vec<ArtifactRef>,
    ) -> ReviewRecord {
        self.auth
            .source
            .create_session_review(
                token,
                &self.workspace,
                id(),
                targets,
                evidence,
                edit(HumanReviewVerdict::Approved, "Checked document"),
            )
            .await
            .unwrap()
    }
    pub async fn request(
        &self,
        token: &str,
        review_id: ReviewId,
        targets: Vec<ArtifactRef>,
        evidence: Vec<ArtifactRef>,
    ) -> Result<ReviewRecord, SessionReviewError> {
        self.auth
            .source
            .create_session_review(
                token,
                &self.workspace,
                review_id,
                targets,
                evidence,
                edit(HumanReviewVerdict::Approved, "Checked document"),
            )
            .await
    }
    pub async fn read(
        &self,
        token: &str,
        reference: ReviewRef,
    ) -> Result<Option<ReviewRecord>, SessionReviewError> {
        self.auth
            .source
            .read_session_review(token, &self.workspace, reference)
            .await
    }
    pub async fn revise(
        &self,
        token: &str,
        reference: ReviewRef,
        edit: HumanReviewEdit,
    ) -> Result<ReviewRecord, SessionReviewError> {
        self.auth
            .source
            .revise_session_review(token, &self.workspace, reference, edit)
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
    pub async fn sql_review(&self, statement: &str) {
        query(statement).execute(&self.review_pool).await.unwrap();
    }
    pub async fn count_review(&self, table: &str) -> i64 {
        assert!([
            "collaboration_reviews",
            "collaboration_review_revisions",
            "collaboration_review_outbox"
        ]
        .contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.review_pool)
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
    pub async fn pause_review_event(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql_review("CREATE FUNCTION pause_session_review_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 771); RETURN NEW; END $$").await;
        self.sql_review("CREATE TRIGGER pause_session_review_event BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION pause_session_review_event()").await;
        let mut blocker = self.review_pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 771)")
            .bind(&self.review_schema)
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
        self.review_pool.close().await;
        self.artifact_pool.close().await;
        for schema in [&self.review_schema, &self.artifact_schema] {
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
