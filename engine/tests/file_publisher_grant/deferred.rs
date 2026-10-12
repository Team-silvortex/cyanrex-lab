use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_deferred_failures_original_relation_pins_and_path_drift_roll_back(
) {
    let f = ready(8).await;
    install_trigger(
        &f,
        CHANGES,
        "INSERT",
        "RAISE EXCEPTION 'synthetic deferred failure';",
        true,
    )
    .await;
    let result = issue(&f, 1, account(1), account(2), validity()).await;
    let fired = hits(&f).await;
    drop_trigger(&f, CHANGES).await;
    issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    f.cleanup().await;
    assert!(fired);
    assert_eq!(
        result,
        Err(FilePublisherGrantStoreError::StorageUnavailable)
    );

    for table in [MARKER, HEADS, CHANGES] {
        let f = ready(8).await;
        let trigger_table = if table == CHANGES { HEADS } else { CHANGES };
        // The replacement is a valid ordinary clone, with all marker OIDs rewritten to match.
        // A final recapture alone could adopt it; the ORIGINAL pin must reject this transaction.
        let body=format!("ALTER TABLE {}.{table} RENAME TO saved_relation; CREATE TABLE {}.{table} (LIKE {}.saved_relation INCLUDING ALL); INSERT INTO {}.{table} SELECT * FROM {}.saved_relation; UPDATE {}.{MARKER} SET schema_oid='{}.{MARKER}'::regclass::oid::bigint,grants_oid='{}.{HEADS}'::regclass::oid::bigint,changes_oid='{}.{CHANGES}'::regclass::oid::bigint; RETURN NEW;",
            f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema);
        install_trigger(&f, trigger_table, "INSERT", &body, true).await;
        let result = issue(&f, 1, account(1), account(2), validity()).await;
        let fired = hits(&f).await;
        drop_trigger(&f, trigger_table).await;
        issue(&f, 1, account(1), account(2), validity())
            .await
            .unwrap();
        f.cleanup().await;
        assert!(fired);
        assert_eq!(result, Err(FilePublisherGrantStoreError::UnsupportedSchema));
    }
    let f = ready(8).await;
    install_trigger(
        &f,
        CHANGES,
        "INSERT",
        "PERFORM set_config('search_path','pg_catalog',true); RETURN NEW;",
        true,
    )
    .await;
    let result = issue(&f, 1, account(1), account(2), validity()).await;
    let fired = hits(&f).await;
    restored(&f).await;
    drop_trigger(&f, CHANGES).await;
    issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    f.cleanup().await;
    assert!(fired);
    assert_eq!(result, Err(FilePublisherGrantStoreError::UnsupportedSchema));

    let f = ready(8).await;
    let old = issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    let payload = serde_json::to_vec(old.grant()).unwrap();
    // Both old head and old historical time change together, so the foreign ledger remains
    // internally valid. Only exact permitted-delta validation rejects this deferred mutation.
    let changed = FilePublisherGrant::new(
        grant(1),
        revision(1),
        scope(),
        account(1),
        FileGrantValidity::new(101, 200).unwrap(),
        FileGrantState::Active,
    )
    .unwrap();
    let hex = serde_json::to_vec(&changed)
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let body=format!("IF NEW.grant_id='{}'::uuid THEN UPDATE {}.{HEADS} SET not_before=101,grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; UPDATE {}.{CHANGES} SET not_before=101,grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; END IF; RETURN NEW;",grant(2),f.schema,grant(1),f.schema,grant(1));
    install_trigger(&f, CHANGES, "INSERT", &body, true).await;
    let result = issue(&f, 2, account(2), account(2), validity()).await;
    let fired = hits(&f).await;
    drop_trigger(&f, CHANGES).await;
    assert_eq!(head_payload(&f, 1).await, payload);
    issue(&f, 2, account(2), account(2), validity())
        .await
        .unwrap();
    f.cleanup().await;
    assert!(fired);
    assert_eq!(result, Err(FilePublisherGrantStoreError::InvalidRecord));

    let f = ready(8).await;
    // Probe proves sync=on at deferred flush only, not crash/power-loss durability or the
    // later COMMIT acknowledgement boundary. The trigger itself is still transactional.
    install_trigger(&f,CHANGES,"INSERT","IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'sync probe'; END IF; RETURN NEW;",true).await;
    configure(&f.pool, "SET synchronous_commit='off'").await;
    issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    assert!(hits(&f).await);
    settings(&f.pool, "read committed", "off", "off").await;
    drop_trigger(&f, CHANGES).await;
    f.cleanup().await;
}
