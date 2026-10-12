use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_deferred_errors_original_pins_foreign_delta_and_sync_probe_are_exact() {
    checked(ready(8).await, |f| {
        Box::pin(async move {
            install_trigger(
                &f,
                CHANGES,
                "INSERT",
                "RAISE EXCEPTION 'synthetic deferred error';",
                true,
            )
            .await;
            let failed = issue(&f, 1, account(1), account(2), account(3), validity()).await;
            let fired = hits(&f).await;
            drop_trigger(&f, CHANGES).await;
            issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
            assert!(fired);
            assert_eq!(
                failed,
                Err(FileRecipientGrantStoreError::StorageUnavailable)
            );
        })
    })
    .await;
    for table in [MARKER, HEADS, CHANGES] {
        checked(ready(8).await,|f|Box::pin(async move{let trigger_table=if table==CHANGES{HEADS}else{CHANGES};let body=format!("ALTER TABLE {}.{table} RENAME TO saved_relation; CREATE TABLE {}.{table}(LIKE {}.saved_relation INCLUDING ALL); INSERT INTO {}.{table} SELECT * FROM {}.saved_relation; UPDATE {}.{MARKER} SET schema_oid='{}.{MARKER}'::regclass::oid::bigint,grants_oid='{}.{HEADS}'::regclass::oid::bigint,changes_oid='{}.{CHANGES}'::regclass::oid::bigint; RETURN NEW;",f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema,f.schema);install_trigger(&f,trigger_table,"INSERT",&body,true).await;let failed=issue(&f,1,account(1),account(2),account(3),validity()).await;let fired=hits(&f).await;drop_trigger(&f,trigger_table).await;issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();assert!(fired);assert_eq!(failed,Err(FileRecipientGrantStoreError::UnsupportedSchema));})).await;
    }
    checked(ready(8).await,|f|Box::pin(async move{let old=issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();let changed=FileRecipientGrant::new(grant(1),revision(1),file(8),account(2),account(3),validity(),FileGrantState::Active).unwrap();let hex=serde_json::to_vec(&changed).unwrap().iter().map(|b|format!("{b:02x}")).collect::<String>();
    // Head + original history still agree: only the allowed exact delta detects this
    // valid FOREIGN file-version mutation. No plaintext file or Published rows are needed.
    let body=format!("IF NEW.grant_id='{}'::uuid THEN UPDATE {}.{HEADS} SET grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; UPDATE {}.{CHANGES} SET grant_payload=decode('{hex}','hex') WHERE grant_id='{}'; END IF; RETURN NEW;",grant(2),f.schema,grant(1),f.schema,grant(1));install_trigger(&f,CHANGES,"INSERT",&body,true).await;let failed=issue(&f,2,account(4),account(5),account(6),validity()).await;let fired=hits(&f).await;drop_trigger(&f,CHANGES).await;assert_eq!(get(&f,1,account(3)).await.unwrap(),Some(old));issue(&f,2,account(4),account(5),account(6),validity()).await.unwrap();assert!(fired);assert_eq!(failed,Err(FileRecipientGrantStoreError::InvalidRecord));})).await;
    checked(ready(8).await, |f| {
        Box::pin(async move {
            let ghost = format!("recipient_absent_{}", Uuid::new_v4().simple());
            let absent: bool = query("SELECT to_regnamespace($1) IS NULL AS yes")
                .bind(&ghost)
                .fetch_one(&f.admin)
                .await
                .unwrap()
                .get("yes");
            assert!(absent);
            let path = format!("{},{ghost}", f.schema);
            install_trigger(
                &f,
                CHANGES,
                "INSERT",
                &format!("PERFORM set_config('search_path','{path}',true); RETURN NEW;"),
                true,
            )
            .await;
            let failed = issue(&f, 1, account(1), account(2), account(3), validity()).await;
            let fired = hits(&f).await;
            restored(&f).await;
            drop_trigger(&f, CHANGES).await;
            issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
            assert!(fired);
            assert_eq!(failed, Err(FileRecipientGrantStoreError::UnsupportedSchema));
        })
    })
    .await;
    checked(ready(8).await,|f|Box::pin(async move{install_trigger(&f,CHANGES,"INSERT","IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'sync probe'; END IF; RETURN NEW;",true).await;configure(&f.pool,"SET synchronous_commit='off'").await;issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();assert!(hits(&f).await);settings(&f.pool,"read committed","off","off").await;drop_trigger(&f,CHANGES).await;})).await;
    // Probe observes the deferred-flush transaction setting, not crash/power-loss/fsync or
    // the later COMMIT acknowledgement boundary. recorded_at is not assumed monotonic.
}
