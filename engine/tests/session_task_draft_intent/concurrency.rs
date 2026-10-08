use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_cancelled_insert_wait_rolls_back_without_publication() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    let before = f.state().await;
    let source = f.source_state().await;
    drop(attempt.register_intent(&f.auth.learner_token, &f.intent_workspace));
    let unpolled_unchanged = before == f.state().await;
    let mut holder = f.pause_insert().await;
    let blocker = backend(&mut holder).await;
    let mut pending = Box::pin(attempt.register_intent(&f.auth.learner_token, &f.intent_workspace));
    let waited = tokio::select! {
        _ = &mut pending => false,
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
    drop(pending);
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    let hit = f.hit().await;
    let after = f.state().await;
    let source_unchanged = source == f.source_state().await;
    let rows = f.count().await;
    let effects = counts(&f).await;
    let ready = attempt.state() == &SessionDraftPublicationState::Ready;
    f.cleanup().await;
    assert!(waited && hit && unpolled_unchanged && source_unchanged && ready);
    assert_eq!(before, after);
    assert_eq!(rows, 0);
    assert_eq!(effects, [0; 6]);
    // This controlled pre-COMMIT cancellation rolls back; it is not a general unknown-outcome proof.
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_pending_writer_serializes_read_and_cancelled_observer_is_read_only()
{
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    let mut holder = f.pause_insert().await;
    let blocker = backend(&mut holder).await;
    let mut writer = Box::pin(attempt.register_intent(&f.auth.learner_token, &f.intent_workspace));
    let mut early_writer = None;
    let writer_pid = tokio::select! {
        result = &mut writer => { early_writer = Some(result); None },
        waiter = blocked_by(&f, blocker) => waiter,
    };
    let mut cancelled_reader_waited = false;
    let mut second_reader_waited = false;
    let mut early_reader = None;
    let mut reader = Box::pin(f.get(&f.auth.learner_token, attempt.task_ref().task_id));
    if let Some(writer_pid) = writer_pid {
        cancelled_reader_waited = tokio::select! {
            result = &mut reader => { early_reader = Some(result); false },
            waiter = blocked_by(&f, writer_pid) => waiter.is_some(),
        };
    }
    drop(reader);
    // A cancelled read must neither cancel the separate writer nor release its lock/alter state.
    let writer_still_blocked: bool = query("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid = $1 AND $2 = ANY(pg_blocking_pids(pid))) AS waiting")
        .bind(writer_pid.unwrap_or(0)).bind(blocker).fetch_one(&f.auth.base.admin).await.unwrap().get("waiting");
    let mut reader = Box::pin(f.get(&f.auth.learner_token, attempt.task_ref().task_id));
    if let Some(writer_pid) = writer_pid {
        second_reader_waited = tokio::select! {
            result = &mut reader => { early_reader = Some(result); false },
            waiter = blocked_by(&f, writer_pid) => waiter.is_some(),
        };
    }
    holder.rollback().await.unwrap();
    let registered = match early_writer {
        Some(result) => result,
        None => writer.as_mut().await,
    };
    drop(writer);
    let observed = match early_reader {
        Some(result) => result,
        None => reader.as_mut().await,
    };
    drop(reader);
    let registered = registered.as_ref().map(receipt).map_err(|error| *error);
    let observed = observed
        .as_ref()
        .map(|item| item.as_ref().map(receipt))
        .map_err(|error| *error);
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    let before = f.state().await;
    let loaded = f
        .get(&f.auth.learner_token, attempt.task_ref().task_id)
        .await;
    let later = loaded
        .as_ref()
        .map(|item| item.as_ref().map(receipt))
        .map_err(|error| *error);
    let unchanged = before == f.state().await;
    let hit = f.hit().await;
    let effects = counts(&f).await;
    f.cleanup().await;
    assert!(
        cancelled_reader_waited && second_reader_waited && writer_still_blocked && hit && unchanged
    );
    let expected = Some(registered.unwrap());
    assert_eq!(observed.unwrap(), expected);
    assert_eq!(later.unwrap(), expected);
    assert_eq!(effects, [0; 6]);
}
