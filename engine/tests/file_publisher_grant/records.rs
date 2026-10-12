use super::{fixture::*, support::*, *};

async fn bad_selected(f: &Fixture) {
    assert_eq!(
        get(f, 1, account(2)).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
    assert_eq!(
        issue(f, 9, account(9), account(9), validity()).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
}
async fn remove_check(f: &Fixture, table: &str, mentions: &str) -> (String, String) {
    let r=query("SELECT conname AS n,pg_get_constraintdef(oid) AS d FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c' AND pg_get_constraintdef(oid) LIKE $2")
        .bind(format!("{}.{table}",f.schema)).bind(format!("%{mentions}%")).fetch_one(&f.pool).await.unwrap();
    let name: String = r.get("n");
    let definition: String = r.get("d");
    sql(f, &format!("ALTER TABLE {table} DROP CONSTRAINT {name}")).await;
    (name, definition)
}
async fn restore_check(f: &Fixture, table: &str, name: &str, definition: &str) {
    sql(
        f,
        &format!("ALTER TABLE {table} ADD CONSTRAINT {name} {definition}"),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_head_and_ordered_history_corruption_fail_closed() {
    let f = ready(16).await;
    issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    let original = head_payload(&f, 1).await;
    for payload in [b"not JSON".to_vec(), [original.as_slice(), b" "].concat()] {
        update_payload(&f, HEADS, 1, 1, &payload).await;
        bad_selected(&f).await;
        update_payload(&f, HEADS, 1, 1, &original).await;
        assert!(get(&f, 1, account(2)).await.unwrap().is_some());
    }
    for (column, value, restore) in [
        (
            "issuer_id",
            format!("'{}'", account(8).principal().principal_id),
            format!("'{}'", account(1).principal().principal_id),
        ),
        (
            "issuer_account_id",
            format!("'{}'", account(8).account_id()),
            format!("'{}'", account(1).account_id()),
        ),
        (
            "subject_account_id",
            format!("'{}'", account(8).account_id()),
            format!("'{}'", account(2).account_id()),
        ),
        ("not_before", "101".into(), "100".into()),
        ("revision", "2".into(), "1".into()),
    ] {
        sql(&f, &format!("UPDATE {HEADS} SET {column}={value}")).await;
        // A mismatching scalar account filter is not decoded as a visible own row.
        if column == "subject_account_id" {
            assert!(get(&f, 1, account(2)).await.unwrap().is_none());
            assert_eq!(
                issue(&f, 9, account(9), account(9), validity()).await,
                Err(FilePublisherGrantStoreError::InvalidRecord)
            );
        } else {
            bad_selected(&f).await;
        }
        sql(&f, &format!("UPDATE {HEADS} SET {column}={restore}")).await;
    }
    for (column, value, restore) in [
        (
            "actor_id",
            format!("'{}'", account(8).principal().principal_id),
            format!("'{}'", account(1).principal().principal_id),
        ),
        (
            "actor_account_id",
            format!("'{}'", account(8).account_id()),
            format!("'{}'", account(1).account_id()),
        ),
        ("not_before", "101".into(), "100".into()),
    ] {
        sql(&f, &format!("UPDATE {CHANGES} SET {column}={value}")).await;
        bad_selected(&f).await;
        sql(&f, &format!("UPDATE {CHANGES} SET {column}={restore}")).await;
    }
    update_payload(&f, CHANGES, 1, 1, b"{}").await;
    bad_selected(&f).await;
    update_payload(&f, CHANGES, 1, 1, &original).await;
    sql(&f, &format!("DELETE FROM {CHANGES}")).await;
    bad_selected(&f).await;
    query(&format!("INSERT INTO {CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) VALUES($1,1,$2,$3,100,200,100,$4)"))
        .bind(grant(1).as_uuid()).bind(account(1).principal().principal_id.as_uuid()).bind(account(1).account_id().as_uuid()).bind(&original).execute(&f.pool).await.unwrap();
    sql(&f, &format!("UPDATE {CHANGES} SET revision=2")).await;
    bad_selected(&f).await;
    sql(&f, &format!("UPDATE {CHANGES} SET revision=1")).await;
    let (name, definition) = remove_check(&f, CHANGES, "recorded_at").await;
    for recorded_at in [-1, 253402300800_i64] {
        sql(
            &f,
            &format!("UPDATE {CHANGES} SET recorded_at={recorded_at}"),
        )
        .await;
        bad_selected(&f).await;
    }
    sql(&f, &format!("UPDATE {CHANGES} SET recorded_at=100")).await;
    restore_check(&f, CHANGES, &name, &definition).await;
    for table in [HEADS, CHANGES] {
        let (name, definition) = remove_check(&f, table, "octet_length").await;
        update_payload(&f, table, 1, 1, &vec![b' '; MAX_FILE_GRANT_JSON_BYTES + 1]).await;
        bad_selected(&f).await;
        update_payload(&f, table, 1, 1, &original).await;
        restore_check(&f, table, &name, &definition).await;
        assert!(get(&f, 1, account(2)).await.unwrap().is_some());
    }
    let revoked = revoke(&f, 1, account(9), revision(1)).await.unwrap();
    sql(
        &f,
        &format!("UPDATE {CHANGES} SET grant_payload=decode('7b7d','hex') WHERE revision=2"),
    )
    .await;
    bad_selected(&f).await;
    update_payload(
        &f,
        CHANGES,
        1,
        2,
        &serde_json::to_vec(revoked.grant()).unwrap(),
    )
    .await;
    assert_eq!(get(&f, 1, account(2)).await.unwrap(), Some(revoked));
    f.cleanup().await;
}
