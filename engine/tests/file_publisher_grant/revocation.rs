use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_revocation_is_one_cas_with_immutable_issuer_and_actor_history() {
    let f = ready(16).await;
    let original = issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    issue(&f, 2, account(3), account(4), validity())
        .await
        .unwrap();
    assert_eq!(
        revoke(&f, 1, account(9), revision(2)).await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    assert_eq!(
        revoke(&f, 99, account(9), revision(1)).await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    let revoked = revoke(&f, 1, account(9), revision(1)).await.unwrap();
    assert_eq!(revoked.issuer(), original.issuer());
    assert_eq!(revoked.grant().id(), original.grant().id());
    assert_eq!(revoked.grant().subject(), original.grant().subject());
    assert_eq!(revoked.grant().workspace(), scope());
    assert_eq!(revoked.grant().validity(), original.grant().validity());
    assert_eq!(revoked.grant().revision(), revision(2));
    assert_eq!(revoked.grant().state(), FileGrantState::Revoked);
    assert!(!revoked.grant().matches(scope(), account(2), 150));
    assert!(
        original.grant().matches(scope(), account(2), 150),
        "copied active metadata is not a current grant authorization"
    );
    let history=query(&format!("SELECT revision,actor_id,actor_account_id,not_before,expires_at,grant_payload,recorded_at FROM {}.{CHANGES} WHERE grant_id=$1 ORDER BY revision",f.schema))
        .bind(grant(1).as_uuid()).fetch_all(&f.admin).await.unwrap();
    assert_eq!(history.len(), 2);
    for (index, (actor, expected)) in [
        (account(1), original.grant()),
        (account(9), revoked.grant()),
    ]
    .into_iter()
    .enumerate()
    {
        let row = &history[index];
        assert_eq!(row.get::<i64, _>("revision"), index as i64 + 1);
        assert_eq!(
            row.get::<Uuid, _>("actor_id"),
            actor.principal().principal_id.as_uuid()
        );
        assert_eq!(
            row.get::<Uuid, _>("actor_account_id"),
            actor.account_id().as_uuid()
        );
        assert_eq!(row.get::<i64, _>("not_before"), 100);
        assert_eq!(row.get::<i64, _>("expires_at"), 200);
        assert_eq!(
            row.get::<Vec<u8>, _>("grant_payload"),
            serde_json::to_vec(expected).unwrap()
        );
        assert!(
            (0..=MAX_FILE_GRANT_UNIX_SECONDS as i64).contains(&row.get::<i64, _>("recorded_at"))
        );
    }
    for actor in [account(1), account(9)] {
        for expected in [revision(1), revision(2), revision(3)] {
            assert_eq!(
                revoke(&f, 1, actor, expected).await,
                Err(FilePublisherGrantStoreError::Conflict)
            );
        }
    }
    assert_eq!(
        issue(&f, 1, account(1), account(2), validity()).await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    let selected = get(&f, 1, account(2)).await.unwrap();
    let (reopened, pool) = f.reopen().await;
    let before = snapshot(&f).await;
    let observed = reopened.get(grant(1), account(2)).await;
    let after = snapshot(&f).await;
    pool.close().await;
    restored(&f).await;
    f.cleanup().await;
    assert_eq!(selected, Some(revoked.clone()));
    assert_eq!(observed.unwrap(), Some(revoked));
    assert_eq!(after, before);
}
