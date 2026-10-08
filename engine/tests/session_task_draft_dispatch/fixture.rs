use super::*;

pub use intent_fixture::ITEMS;
pub type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
pub struct Fixture {
    pub base: intent_fixture::Fixture,
}
impl std::ops::Deref for Fixture {
    type Target = intent_fixture::Fixture;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl Fixture {
    pub async fn ready() -> Self {
        let base = content_fixture::Fixture::ready().await;
        let journal_schema = format!("draft_dispatch_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {journal_schema}"))
            .execute(&base.auth.base.admin)
            .await
            .unwrap();
        let journal_pool = input_fixture::pool_for(&journal_schema).await;
        let installation = Uuid::new_v4();
        let journal = DraftIntentJournal::new(journal_pool.clone(), scope(), installation).unwrap();
        if let Err(error) = journal.install_empty_dispatch_namespace().await {
            // The initial TDD installer stub may reject before the dispatch behavior is reached.
            // Still remove all fixture namespaces and the private blob directory before failing.
            journal_pool.close().await;
            query(&format!("DROP SCHEMA {journal_schema} CASCADE"))
                .execute(&base.auth.base.admin)
                .await
                .unwrap();
            drop(journal);
            base.cleanup().await;
            panic!("fresh dispatch namespace installation must succeed: {error:?}");
        }
        let intent_workspace = SessionDraftIntentWorkspace::new(
            &journal_schema,
            journal.clone(),
            base.workspace.clone(),
        )
        .unwrap();
        Self {
            base: intent_fixture::Fixture {
                base,
                journal,
                journal_pool,
                journal_schema,
                installation,
                intent_workspace,
            },
        }
    }
    pub async fn journaled(
        &self,
        items: &[(&str, &str, &str)],
    ) -> SessionJournaledTaskDraftPublication {
        let attempt = prepare(self, items);
        attempt
            .register_intent(&self.auth.learner_token, &self.intent_workspace)
            .await
            .unwrap();
        attempt
            .into_journaled(self.intent_workspace.clone())
            .unwrap()
    }
    pub async fn steps(&self) -> Vec<(i32, Uuid, bool)> {
        query("SELECT ordinal, nonce, committed FROM collaboration_draft_dispatch_steps ORDER BY ordinal")
            .fetch_all(&self.journal_pool).await.unwrap().into_iter()
            .map(|row| (row.get("ordinal"), row.get("nonce"), row.get("committed"))).collect()
    }
    pub async fn step_fault(&self, phase: &str, body: &str, deferred: bool) {
        assert!(["INSERT", "UPDATE"].contains(&phase));
        self.sql("CREATE SEQUENCE dispatch_fault_calls").await;
        self.sql(&format!(
            "CREATE FUNCTION dispatch_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.dispatch_fault_calls'); {body} RETURN NEW; END $$",
            self.journal_schema
        ))
        .await;
        self.sql(&if deferred {
            format!("CREATE CONSTRAINT TRIGGER dispatch_fault AFTER {phase} ON collaboration_draft_dispatch_steps DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION dispatch_fault()")
        } else {
            format!("CREATE TRIGGER dispatch_fault BEFORE {phase} ON collaboration_draft_dispatch_steps FOR EACH ROW EXECUTE FUNCTION dispatch_fault()")
        }).await;
    }
    pub async fn step_hit(&self) -> bool {
        query("SELECT is_called FROM dispatch_fault_calls")
            .fetch_one(&self.journal_pool)
            .await
            .unwrap()
            .get("is_called")
    }
    pub async fn resource_fault(&self, task: bool, body: &str, deferred: bool) {
        let (pool, namespace, table) = if task {
            (
                &self.task_pool,
                &self.task_schema,
                "collaboration_task_outbox",
            )
        } else {
            (
                &self.artifact_pool,
                &self.artifact_schema,
                "collaboration_artifact_outbox",
            )
        };
        for sql in [
            "CREATE SEQUENCE resource_fault_calls".to_owned(),
            format!("CREATE FUNCTION resource_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{namespace}.resource_fault_calls'); {body} RETURN NEW; END $$"),
            if deferred { format!("CREATE CONSTRAINT TRIGGER resource_fault AFTER INSERT ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION resource_fault()") }
            else { format!("CREATE TRIGGER resource_fault BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION resource_fault()") },
        ] { query(&sql).execute(pool).await.unwrap(); }
    }
    pub async fn resource_hit(&self, task: bool) -> bool {
        query("SELECT is_called FROM resource_fault_calls")
            .fetch_one(if task {
                &self.task_pool
            } else {
                &self.artifact_pool
            })
            .await
            .unwrap()
            .get("is_called")
    }
    pub async fn pause_phase(&self, phase: &str) -> Tx<'_> {
        self.step_fault(
            phase,
            "PERFORM pg_advisory_xact_lock(hashtext(TG_TABLE_SCHEMA), 883);",
            false,
        )
        .await;
        let mut tx = self.journal_pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext($1), 883)")
            .bind(&self.journal_schema)
            .execute(&mut *tx)
            .await
            .unwrap();
        tx
    }
    pub async fn cleanup(self) {
        self.base.cleanup().await;
    }
}

pub fn stopped() -> SessionDraftDispatchError {
    SessionDraftDispatchError::Publication(SessionDraftPublicationError::Stopped)
}
pub fn unknown(attempt: &SessionJournaledTaskDraftPublication) -> bool {
    matches!(
        attempt.state(),
        SessionDraftPublicationState::Unconfirmed(_)
    )
}
