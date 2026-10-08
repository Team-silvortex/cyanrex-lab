use super::*;
async fn blocked(f: &Fixture, waiter: i32, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = query("SELECT $1 = ANY(pg_blocking_pids($2)) AS waiting")
                .bind(blocker)
                .bind(waiter)
                .fetch_one(&f.auth.base.admin)
                .await
                .unwrap()
                .get("waiting");
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_holds_original_journal_guard_across_resource_wait() {
    for mode in ["intent", "nonce", "relation"] {
        let f = Fixture::ready().await;
        let attempt = complete(&f, ITEMS).await;
        let before = snapshot(&f).await;
        let mut resource_holder = resource_lock(&f, false).await;
        let blocker = backend(&mut resource_holder).await;
        let mut reader = Box::pin(observe(
            &f,
            &f.auth.learner_token,
            attempt.task_ref().task_id,
            0,
        ));
        let observer =
            tokio::select! { _ = &mut reader => None, waiter = blocked_by(&f, blocker) => waiter };
        let mut writer_tx = f.journal_pool.begin().await.unwrap();
        let writer_pid = backend(&mut writer_tx).await;
        let sql = match mode {
            "intent" => "UPDATE collaboration_draft_intents SET checkpoint = convert_to('{}', 'UTF8')",
            "nonce" => "UPDATE collaboration_draft_dispatch_steps SET nonce = '00000000-0000-0000-0000-000000000000' WHERE ordinal = 0",
            _ => "ALTER TABLE collaboration_draft_dispatch_steps RENAME TO journal_steps_original",
        };
        let mut writer = Box::pin(query(sql).execute(&mut *writer_tx));
        let mut early_writer = None;
        let protected = if let Some(observer) = observer {
            tokio::select! { result = &mut writer => { early_writer = Some(result); false }, value = blocked(&f, writer_pid, observer) => value }
        } else {
            false
        };
        // External changes cannot cross the held row/relation guards. Cancelling this reader
        // releases those guards; it never returns a mixed snapshot or an inferred receipt.
        drop(reader);
        resource_holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        let written = match early_writer {
            Some(value) => value,
            None => writer.as_mut().await,
        };
        drop(writer);
        if mode == "relation" && written.is_ok() {
            for sql in [
                "CREATE TABLE collaboration_draft_dispatch_steps (LIKE journal_steps_original INCLUDING ALL)".to_owned(),
                "ALTER TABLE collaboration_draft_dispatch_steps ADD FOREIGN KEY (task_id) REFERENCES collaboration_draft_intents(task_id)".to_owned(),
                "INSERT INTO collaboration_draft_dispatch_steps SELECT * FROM journal_steps_original".to_owned(),
                format!("UPDATE collaboration_draft_intent_schema SET installation_id = '{}'", Uuid::new_v4()),
            ] { query(&sql).execute(&mut *writer_tx).await.unwrap(); }
        }
        writer_tx.commit().await.unwrap();
        f.assert_pool_restored().await;
        let subsequent = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
        let after = snapshot(&f).await;
        let resource_intact =
            before.0[2] == after.0[2] && before.0[4..] == after.0[4..] && before.1 == after.1;
        f.cleanup().await;
        assert!(protected && written.is_ok() && resource_intact, "{mode}");
        assert_eq!(
            subsequent,
            Err(SessionDraftJournalObservationError::Intent(
                SessionDraftIntentError::Journal(if mode == "relation" {
                    DraftIntentJournalError::IncarnationMismatch
                } else {
                    DraftIntentJournalError::InvalidRecord
                })
            ))
        );
        // The OID-revisit source guard, not this blocked external writer, covers same-read pin reuse.
    }
}
