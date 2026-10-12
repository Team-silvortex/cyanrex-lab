use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_revocation_is_single_cas_and_preserves_file_parties_issuer_history() {
    checked(ready(16).await, |f| Box::pin(async move {
    let original = issue(&f, 1, account(1), account(2), account(3), validity())
        .await
        .unwrap();
    issue(&f, 2, account(4), account(5), account(6), validity())
        .await
        .unwrap();
    for expected in [revision(2), revision(3)] {
        assert_eq!(
            revoke(&f, 1, account(9), expected).await,
            Err(FileRecipientGrantStoreError::Conflict)
        );
    }
    assert_eq!(
        revoke(&f, 99, account(9), revision(1)).await,
        Err(FileRecipientGrantStoreError::Conflict)
    );
    let retired = revoke(&f, 1, account(9), revision(1)).await.unwrap();
    assert_eq!(retired.issuer(), original.issuer());
    assert_eq!(retired.grant().file(), original.grant().file());
    assert_eq!(retired.grant().publisher(), original.grant().publisher());
    assert_eq!(retired.grant().recipient(), original.grant().recipient());
    assert_eq!(retired.grant().validity(), original.grant().validity());
    assert_eq!(retired.grant().revision(), revision(2));
    assert_eq!(retired.grant().state(), FileGrantState::Revoked);
    assert!(!retired
        .grant()
        .matches(&file(1), account(2), account(3), 150));
    assert!(
        original
            .grant()
            .matches(&file(1), account(2), account(3), 150),
        "copied Active metadata is not current authority"
    );
    let before = f.snapshot().await;
    let history=query(&format!("SELECT revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {}.{CHANGES} WHERE grant_id=$1 ORDER BY revision",f.schema)).bind(grant(1).as_uuid()).fetch_all(&f.admin).await.unwrap();
    assert_eq!(f.snapshot().await, before);
    assert_eq!(history.len(), 2);
    for (i, (actor, expected)) in [
        (account(1), original.grant()),
        (account(9), retired.grant()),
    ]
    .into_iter()
    .enumerate()
    {
        let row = &history[i];
        assert_eq!(row.get::<i64, _>("revision"), i as i64 + 1);
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
                Err(FileRecipientGrantStoreError::Conflict)
            );
        }
    }
    assert_eq!(
        issue(&f, 1, account(1), account(2), account(3), validity()).await,
        Err(FileRecipientGrantStoreError::Conflict)
    );
    assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(retired));
    })).await;
}
