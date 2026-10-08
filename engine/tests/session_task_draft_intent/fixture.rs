use super::*;

pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub const ITEMS: &[(&str, &str, &str)] = &[
    ("同名.txt", "future.lang", "private 😀\r\ntext"),
    ("同名.txt", "text", "private 😀\r\ntext"),
];

pub struct Fixture {
    pub base: content_fixture::Fixture,
    pub journal: DraftIntentJournal,
    pub journal_pool: PgPool,
    pub journal_schema: String,
    pub installation: Uuid,
    pub intent_workspace: SessionDraftIntentWorkspace,
}
impl std::ops::Deref for Fixture {
    type Target = content_fixture::Fixture;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl Fixture {
    pub async fn ready() -> Self {
        let base = content_fixture::Fixture::ready().await;
        let journal_schema = format!("draft_intents_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {journal_schema}"))
            .execute(&base.auth.base.admin)
            .await
            .unwrap();
        let journal_pool = input_fixture::pool_for(&journal_schema).await;
        let installation = Uuid::new_v4();
        let journal = DraftIntentJournal::new(journal_pool.clone(), scope(), installation).unwrap();
        journal.install_empty_namespace().await.unwrap();
        let intent_workspace = SessionDraftIntentWorkspace::new(
            &journal_schema,
            journal.clone(),
            base.workspace.clone(),
        )
        .unwrap();
        Self {
            base,
            journal,
            journal_pool,
            journal_schema,
            installation,
            intent_workspace,
        }
    }
    pub async fn sql(&self, sql: &str) {
        query(sql).execute(&self.journal_pool).await.unwrap();
    }
    pub async fn count(&self) -> i64 {
        query("SELECT count(*) AS n FROM collaboration_draft_intents")
            .fetch_one(&self.journal_pool)
            .await
            .unwrap()
            .get("n")
    }
    pub async fn get(
        &self,
        token: &str,
        task: TaskId,
    ) -> Result<Option<SessionDraftPublicationIntent>, SessionDraftIntentError> {
        self.auth
            .source
            .get_session_draft_intent(token, &self.intent_workspace, task)
            .await
    }
    pub async fn state(&self) -> String {
        query("SELECT md5(COALESCE(string_agg(row_to_json(t)::text, '' ORDER BY row_to_json(t)::text), '')) AS hash FROM collaboration_draft_intents t")
            .fetch_one(&self.journal_pool).await.unwrap().get("hash")
    }
    pub async fn source_state(&self) -> String {
        query("SELECT md5(COALESCE(string_agg(value, '' ORDER BY value), '')) AS hash FROM
            (SELECT row_to_json(u)::text AS value FROM users u UNION ALL SELECT row_to_json(s)::text FROM sessions s) t")
            .fetch_one(&self.auth.base.pool).await.unwrap().get("hash")
    }
    pub async fn metadata(&self) -> String {
        query("SELECT row_to_json(t)::text AS json FROM collaboration_draft_intent_schema t")
            .fetch_one(&self.journal_pool)
            .await
            .unwrap()
            .get("json")
    }
    pub async fn fault(&self, body: &str, deferred: bool) {
        self.sql("CREATE SEQUENCE intent_fault_calls").await;
        self.sql(&format!(
            "CREATE FUNCTION intent_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.intent_fault_calls'); {body} RETURN NEW; END $$",
            self.journal_schema
        ))
        .await;
        self.sql(if deferred {
            "CREATE CONSTRAINT TRIGGER intent_fault AFTER INSERT ON collaboration_draft_intents DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION intent_fault()"
        } else {
            "CREATE TRIGGER intent_fault BEFORE INSERT ON collaboration_draft_intents FOR EACH ROW EXECUTE FUNCTION intent_fault()"
        }).await;
    }
    pub async fn hit(&self) -> bool {
        query("SELECT is_called FROM intent_fault_calls")
            .fetch_one(&self.journal_pool)
            .await
            .unwrap()
            .get("is_called")
    }
    pub async fn pause_insert(&self) -> Tx<'_> {
        self.fault(
            "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 877);",
            false,
        )
        .await;
        self.sql("DROP TRIGGER intent_fault ON collaboration_draft_intents")
            .await;
        self.sql("CREATE TRIGGER intent_fault AFTER INSERT ON collaboration_draft_intents FOR EACH ROW EXECUTE FUNCTION intent_fault()").await;
        let mut holder = self.journal_pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 877)")
            .bind(&self.journal_schema)
            .execute(&mut *holder)
            .await
            .unwrap();
        holder
    }
    pub async fn schema_lock(&self) -> Tx<'_> {
        let mut holder = self.journal_pool.begin().await.unwrap();
        query("SELECT singleton FROM collaboration_draft_intent_schema FOR UPDATE")
            .fetch_one(&mut *holder)
            .await
            .unwrap();
        holder
    }
    pub async fn cleanup(self) {
        self.journal_pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.journal_schema))
            .execute(&self.base.auth.base.admin)
            .await
            .unwrap();
        drop(self.intent_workspace);
        drop(self.journal);
        self.base.cleanup().await;
    }
}

pub fn receipt(
    value: &SessionDraftPublicationIntent,
) -> (TaskRef, PrincipalRef, LegacyAccountId, Vec<u8>) {
    (
        value.task_ref(),
        value.owner(),
        value.account_id(),
        value.checkpoint().to_json().unwrap(),
    )
}
