use super::{
    fixture::*,
    publisher_fixture::{grant, validity, CHANGES},
    support::*,
    *,
};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_manager_crud_cas_derives_actor_and_preserves_original_issuer() {
    let f = Fixture::ready().await;
    f.base.pool.close().await;
    // The original source transaction is sufficient: the independently installed pool is closed.
    let active = issue(&f, 1, f.target()).await.unwrap();
    assert_eq!(active.issuer(), f.manager());
    assert_eq!(
        read(&f, &f.base.auth.manager_token, 1).await.unwrap(),
        Some(active.clone())
    );
    assert_eq!(read(&f, &f.base.auth.learner_token, 1).await.unwrap(), None);
    assert_eq!(
        issue(&f, 1, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::Conflict))
    );
    assert_eq!(
        revoke(&f, 1, revision(2)).await,
        Err(grant_error(FilePublisherGrantStoreError::Conflict))
    );
    assert_eq!(
        revoke(&f, 99, revision(1)).await,
        Err(grant_error(FilePublisherGrantStoreError::Conflict))
    );
    let revoked = revoke(&f, 1, revision(1)).await.unwrap();
    assert_eq!(revoked.issuer(), active.issuer());
    assert_eq!(revoked.grant().state(), FileGrantState::Revoked);
    assert_eq!(revoked.grant().subject(), active.grant().subject());
    assert_eq!(revoked.grant().validity(), active.grant().validity());
    for expected in [revision(1), revision(2), revision(3)] {
        assert_eq!(
            revoke(&f, 1, expected).await,
            Err(grant_error(FilePublisherGrantStoreError::Conflict))
        );
    }
    let histories = query(&format!(
        "SELECT actor_id,actor_account_id FROM {}.{CHANGES} WHERE grant_id=$1 ORDER BY revision",
        f.base.schema
    ))
    .bind(grant(1).as_uuid())
    .fetch_all(&f.base.auth.base.admin)
    .await
    .unwrap();
    assert_eq!(histories.len(), 2);
    for h in histories {
        assert_eq!(
            h.get::<Uuid, _>("actor_id"),
            f.manager().principal().principal_id.as_uuid()
        );
        assert_eq!(
            h.get::<Uuid, _>("actor_account_id"),
            f.manager().account_id().as_uuid()
        );
    }
    assert_eq!(
        read(&f, &f.base.auth.manager_token, 1).await.unwrap(),
        Some(revoked)
    );
    restored(&f).await;
    f.cleanup().await;

    let f = Fixture::ready().await;
    let original = f
        .base
        .store
        .issue(grant(2), f.target(), f.target(), validity())
        .await
        .unwrap();
    let revoked = revoke(&f, 2, revision(1)).await.unwrap();
    assert_eq!(revoked.issuer(), original.issuer());
    assert_ne!(revoked.issuer(), f.manager());
    let h = query(&format!(
        "SELECT actor_id,actor_account_id FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=2",
        f.base.schema
    ))
    .bind(grant(2).as_uuid())
    .fetch_one(&f.base.auth.base.admin)
    .await
    .unwrap();
    assert_eq!(
        h.get::<Uuid, _>("actor_id"),
        f.manager().principal().principal_id.as_uuid()
    );
    assert_eq!(
        h.get::<Uuid, _>("actor_account_id"),
        f.manager().account_id().as_uuid()
    );
    f.cleanup().await;
}
