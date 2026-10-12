use super::cleanup::checked;
use super::{fixture::*, support::*, *};
async fn bad(f: &Fixture) {
    assert_eq!(
        get(f, 1, account(3)).await,
        Err(FileRecipientGrantStoreError::InvalidRecord)
    );
    assert_eq!(
        issue(f, 1, account(1), account(2), account(3), validity()).await,
        Err(FileRecipientGrantStoreError::InvalidRecord)
    );
    assert_eq!(
        issue(f, 9, account(9), account(9), account(9), validity()).await,
        Err(FileRecipientGrantStoreError::InvalidRecord)
    );
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_complete_history_foreign_ledger_and_excess_inventory_precede_conflict_quota(
) {
    checked(ready(2).await,|f|Box::pin(async move{let own=issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();issue(&f,2,account(4),account(5),account(6),validity()).await.unwrap();let original=head_payload(&f,1).await;
    for(column,value,restore)in[("actor_id",format!("'{}'",account(8).principal().principal_id),format!("'{}'",account(1).principal().principal_id)),("actor_account_id",format!("'{}'",account(8).account_id()),format!("'{}'",account(1).account_id())),("not_before","101".into(),"100".into()),("revision","2".into(),"1".into())]{sql(&f,&format!("UPDATE {CHANGES} SET {column}={value} WHERE grant_id='{}'",grant(1))).await;bad(&f).await;sql(&f,&format!("UPDATE {CHANGES} SET {column}={restore} WHERE grant_id='{}'",grant(1))).await;}
    for raw in[b"{}".to_vec(),[original.as_slice(),b" "].concat()]{update_payload(&f,CHANGES,1,1,&raw).await;bad(&f).await;update_payload(&f,CHANGES,1,1,&original).await;}
    sql(&f,&format!("DELETE FROM {CHANGES} WHERE grant_id='{}'",grant(1))).await;bad(&f).await;
    query(&format!("INSERT INTO {CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,100,200,100,$4)")).bind(grant(1).as_uuid()).bind(account(1).principal().principal_id.as_uuid()).bind(account(1).account_id().as_uuid()).bind(&original).execute(&f.pool).await.unwrap();
    let(name,definition)=remove_check(&f,CHANGES,"recorded_at").await;for time in[-1,253402300800_i64]{sql(&f,&format!("UPDATE {CHANGES} SET recorded_at={time} WHERE grant_id='{}'",grant(1))).await;bad(&f).await;}sql(&f,&format!("UPDATE {CHANGES} SET recorded_at=100 WHERE grant_id='{}'",grant(1))).await;restore_check(&f,CHANGES,&name,&definition).await;
    let(name,definition)=remove_check(&f,CHANGES,"octet_length").await;update_payload(&f,CHANGES,1,1,&vec![b' ';MAX_FILE_GRANT_JSON_BYTES+1]).await;bad(&f).await;update_payload(&f,CHANGES,1,1,&original).await;restore_check(&f,CHANGES,&name,&definition).await;
    let foreign=head_payload(&f,2).await;update_payload(&f,HEADS,2,1,b"{}").await;assert_eq!(get(&f,1,account(3)).await.unwrap(),Some(own.clone()));assert_eq!(get(&f,2,account(3)).await.unwrap(),None);assert_eq!(issue(&f,1,account(1),account(2),account(3),validity()).await,Err(FileRecipientGrantStoreError::InvalidRecord));assert_eq!(revoke(&f,1,account(9),revision(1)).await,Err(FileRecipientGrantStoreError::InvalidRecord));update_payload(&f,HEADS,2,1,&foreign).await;
    query(&format!("INSERT INTO {CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT $1,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {CHANGES} WHERE grant_id=$2")).bind(grant(99).as_uuid()).bind(grant(2).as_uuid()).execute(&f.pool).await.unwrap();assert_eq!(issue(&f,1,account(1),account(2),account(3),validity()).await,Err(FileRecipientGrantStoreError::InvalidRecord));sql(&f,&format!("DELETE FROM {CHANGES} WHERE grant_id='{}'",grant(99))).await;
    let retired=revoke(&f,1,account(9),revision(1)).await.unwrap();update_payload(&f,CHANGES,1,2,b"{}").await;assert_eq!(get(&f,1,account(3)).await,Err(FileRecipientGrantStoreError::InvalidRecord));update_payload(&f,CHANGES,1,2,&serde_json::to_vec(retired.grant()).unwrap()).await;assert_eq!(get(&f,1,account(3)).await.unwrap(),Some(retired));})).await;
    checked(ready(256).await, |f| {
        Box::pin(async move {
            for seed in 1..=257 {
                insert_raw(&f, seed, account(1), account(2), account(3)).await;
            }
            let before = f.snapshot().await;
            let excess = f
                .store
                .issue(
                    grant(999),
                    account(1),
                    file(999),
                    account(2),
                    account(3),
                    validity(),
                )
                .await;
            let after = f.snapshot().await;
            assert_eq!(excess, Err(FileRecipientGrantStoreError::InvalidRecord));
            assert_eq!(after, before);
        })
    })
    .await;
}
