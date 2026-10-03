use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_duplicate_creation_and_competing_edits_have_one_winner() {
    let f = Fixture::ready().await;
    let (one, two) = tokio::join!(
        f.store.create(reference().task_id, owner(), manifest()),
        f.store.create(reference().task_id, owner(), manifest())
    );
    assert!(matches!(
        (one, two),
        (Ok(_), Err(TaskStoreError::Conflict)) | (Err(TaskStoreError::Conflict), Ok(_))
    ));
    let first = content("First edit", vec![artifact()]);
    let second = content("Second edit", vec![]);
    let (one, two) = tokio::join!(
        f.store.replace(reference(), owner(), rev(1), first.clone()),
        f.store
            .replace(reference(), owner(), rev(1), second.clone())
    );
    let winner = match (one, two) {
        (Ok(value), Err(TaskStoreError::StaleRevision)) => {
            assert_eq!(value.manifest, first);
            value
        }
        (Err(TaskStoreError::StaleRevision), Ok(value)) => {
            assert_eq!(value.manifest, second);
            value
        }
        other => panic!("unexpected competing edits: {other:?}"),
    };
    assert_eq!(winner.task.revision, rev(2));
    f.unchanged(&winner, 2).await;
    assert_eq!(f.count("collaboration_tasks").await, 1);
    f.recorded(&winner).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_edit_and_status_race_share_one_revision_head() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    let desired = content("Edited", vec![]);
    let (one, two) = tokio::join!(
        f.store
            .replace(reference(), owner(), rev(1), desired.clone()),
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
    );
    let winner = match (one, two) {
        (Ok(value), Err(TaskStoreError::StaleRevision)) => {
            assert_eq!(value.manifest, desired);
            assert_eq!(value.task.status, TaskStatus::Draft);
            value
        }
        (Err(TaskStoreError::StaleRevision), Ok(value)) => {
            assert_eq!(value.manifest, original.manifest);
            assert_eq!(value.task.status, TaskStatus::Ready);
            value
        }
        other => panic!("unexpected edit/transition race: {other:?}"),
    };
    assert_eq!(winner.task.revision, rev(2));
    f.unchanged(&winner, 2).await;
    f.recorded(&winner).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_readers_observe_one_committed_snapshot_during_an_edit() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .replace(
                reference(),
                owner(),
                rev(1),
                content("Edited atomically", vec![]),
            )
            .await
    });
    f.wait_blocked().await;
    let observed = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        f.store.get(reference(), owner()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(observed, Some(original));
    blocker.rollback().await.unwrap();
    let after = pending.await.unwrap().unwrap();
    assert_eq!(after.task.revision, rev(2));
    f.unchanged(&after, 2).await;
    f.recorded(&after).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_writers_wait_for_metadata_before_admission() {
    let f = Fixture::ready().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("UPDATE collaboration_task_schema SET version = 99")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let store = f.store.clone();
    let pending =
        tokio::spawn(async move { store.create(reference().task_id, owner(), manifest()).await });
    f.wait_blocked().await;
    assert!(!pending.is_finished());
    blocker.commit().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_cancelled_event_wait_rolls_back_and_restores_the_pool() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .replace(
                reference(),
                owner(),
                rev(1),
                content("Cancelled edit", vec![]),
            )
            .await
    });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    f.assert_pool_restored().await;
    f.unchanged(&original, 1).await;
    f.recorded(&original).await;
    f.cleanup().await;
}
