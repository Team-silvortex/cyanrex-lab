use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_cancelled_deferred_commit_wait_is_terminal_and_observed_rollback_is_not_retry_authority(
) {
    let f = Fixture::with_limits(17, 1, 8).await;
    f.grants.issue(1, f.actor()).await;
    fault(
        &f,
        "PERFORM pg_advisory_xact_lock(78129,72331); RETURN NEW;",
        true,
    )
    .await;
    let mut a = prepare(&f, 1).await;
    let mut spare = prepare(&f, 1).await;
    let before = f.snapshot().await;
    let mut holder = f.grants.auth.base.admin.begin().await.unwrap();
    let pid = backend(&mut holder).await;
    query("SELECT pg_advisory_xact_lock(78129,72331)")
        .execute(&mut *holder)
        .await
        .unwrap();
    let mut pending = Box::pin(publish(&f, &mut a));
    let waited = tokio::select! {_=&mut pending=>false,b=blocked(&f,pid)=>b};
    let uncommitted = f.snapshot().await;
    let reached = hit(&f).await;
    let capacity = publish(&f, &mut spare).await;
    let spare_state = spare.state();
    let prepare_capacity = f
        .publisher
        .prepare(upload(1), f.actor().principal(), f.actor().account_id())
        .await
        .err();
    drop(pending);
    let state = a.state();
    holder.rollback().await.unwrap();
    drain(&f).await;
    let after = f.snapshot().await;
    let same = publish(&f, &mut a).await;
    remove_fault(&f).await;
    // This independent complete snapshot, not cancellation itself, establishes this fixture's rollback.
    // A generally lost ACK may already have a Published row and cannot authorize an automatic retry.
    let healthy_result = publish(&f, &mut spare).await;
    let spare_final = spare.state();
    let final_snapshot = f.snapshot().await;
    if healthy_result.is_ok() {
        delta(&f, &after, &final_snapshot, 1);
    }
    f.cleanup().await;
    assert!(waited && reached);
    assert_eq!(uncommitted, before);
    assert_eq!(after, before);
    assert_eq!(
        capacity.err(),
        Some(publisher_error(
            FileUploadPublicationError::CapacityExhausted
        ))
    );
    assert_eq!(spare_state, FileUploadPublicationState::Ready);
    assert_eq!(
        prepare_capacity,
        Some(FileUploadPublicationError::CapacityExhausted)
    );
    assert_eq!(state, FileUploadPublicationState::Unknown);
    assert_eq!(
        same.err(),
        Some(publisher_error(FileUploadPublicationError::InvalidInput))
    );
    assert!(healthy_result.is_ok());
    assert_eq!(spare_final, FileUploadPublicationState::Confirmed);
}
