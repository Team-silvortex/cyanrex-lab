use super::{fixture::*, publication::Published, support::*, *};
use std::fs;

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_owner_metadata_is_private_and_uses_only_source_transactions() {
    let f = Fixture::ready().await;
    let own = reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
        .await
        .unwrap();
    assert_eq!(own.owner(), f.auth.learner.principal.reference);
    assert_eq!(own.account_id(), f.auth.learner.account_id);
    assert_eq!(
        get_upload(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(own.clone())
    );
    assert_eq!(
        get_upload(&f, &f.auth.manager_token, 1).await.unwrap(),
        None,
        "teacher/manager status grants no other-owner metadata access"
    );
    assert_eq!(
        get_upload(&f, &f.auth.learner_token, 99).await.unwrap(),
        None
    );
    assert_eq!(
        reserve(&f, &f.auth.manager_token, 1, &manifest(17, 1)).await,
        Err(SessionFileMetadataError::Upload(
            FileUploadStoreError::Conflict
        ))
    );
    let other = reserve(&f, &f.auth.manager_token, 2, &manifest(17, 2))
        .await
        .unwrap();
    assert_eq!(other.owner(), f.auth.manager.principal.reference);
    assert_eq!(other.account_id(), f.auth.manager.account_id);
    assert_eq!(
        get_upload(&f, &f.auth.learner_token, 2).await.unwrap(),
        None
    );
    assert_eq!(
        get_upload(&f, &f.auth.manager_token, 2).await.unwrap(),
        Some(other)
    );
    let published = Published::empty(&f, &f.auth.learner_token, 3).await;
    let journal = published.rows(&f).await;
    let expected = f
        .publications
        .get(
            upload(3),
            f.auth.learner.principal.reference,
            f.auth.learner.account_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(expected.manifest(), &published.manifest);
    assert_eq!(
        get_publication(&f, &f.auth.learner_token, 3).await.unwrap(),
        Some(expected.clone())
    );
    assert_eq!(
        get_publication(&f, &f.auth.manager_token, 3).await.unwrap(),
        None
    );
    assert_eq!(
        get_publication(&f, &f.auth.learner_token, 99)
            .await
            .unwrap(),
        None
    );
    // These handles' independent installation pools and root become unavailable. Session commands
    // still borrow their one auth/source transaction, and publication metadata does not reopen FS.
    f.upload_pool.close().await;
    f.publication_pool.close().await;
    published.pool.close().await;
    fs::rename(
        &published.root,
        published.directory.join("missing-old-root"),
    )
    .unwrap();
    let before = snapshot(&f).await;
    assert_eq!(
        get_upload(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(own)
    );
    assert_eq!(
        get_publication(&f, &f.auth.learner_token, 3).await.unwrap(),
        Some(expected)
    );
    reserve(&f, &f.auth.learner_token, 4, &manifest(17, 4))
        .await
        .unwrap();
    let after = snapshot(&f).await;
    assert_eq!(after.rows[0], before.rows[0]);
    assert_eq!(after.rows[2], before.rows[2]);
    assert_eq!(published.rows(&f).await, journal);
    assert!(fs::read_dir(published.directory.join("missing-old-root"))
        .unwrap()
        .next()
        .is_none());
    f.pool_restored().await;
    published.cleanup(&f).await;
    f.cleanup().await;
}
