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
pub fn reviewer() -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id: "d7ed9514-330f-4cd7-9a30-32741ee8d0cf".parse().unwrap(),
    }
}
pub fn artifact() -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}
pub fn policy() -> VersionedName {
    VersionedName {
        name: "sample.document.editorial".parse().unwrap(),
        version: 1.try_into().unwrap(),
    }
}
pub fn edit(verdict: HumanReviewVerdict, comment: &str) -> HumanReviewEdit {
    HumanReviewEdit::new(verdict, comment).unwrap()
}
pub fn draft(target: ArtifactRef) -> ReviewDraft {
    ReviewDraft::human(
        vec![target],
        vec![],
        policy(),
        edit(HumanReviewVerdict::Approved, "Checked text"),
    )
    .unwrap()
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
    pub store: ReviewStore,
    pub schema: String,
}
impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable database required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("review_work_{}", uuid::Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = pool_for(&url, schema.clone(), 5).await;
        let store = ReviewStore::new(pool.clone(), scope());
        Self {
            admin,
            pool,
            store,
            schema,
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
            "collaboration_reviews",
            "collaboration_review_revisions",
            "collaboration_review_outbox"
        ]
        .contains(&table));
        query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("n")
    }
    pub async fn pause_events(
        &self,
    ) -> sqlx_core::transaction::Transaction<'_, sqlx_postgres::Postgres> {
        self.sql(
            "CREATE FUNCTION pause_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 731); RETURN NEW; END $$",
        )
        .await;
        self.sql("CREATE TRIGGER pause_event BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION pause_event()").await;
        let mut blocker = self.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 731)")
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
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
