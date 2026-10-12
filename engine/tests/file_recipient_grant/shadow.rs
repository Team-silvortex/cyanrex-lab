use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_grant_raw_path_shadow_and_fresh_marker_drift_are_rejected() {
    let (extra_get, extra_issue, extra_after, before) = checked(ready(8).await, |f| {
        Box::pin(async move {
            for path in [f.schema.clone(), format!("\"{}\"", f.schema)] {
                configure(&f.pool, &format!("SET search_path={path}")).await;
                assert_eq!(get(&f, 99, account(3)).await.unwrap(), None);
            }
            configure(&f.pool, &format!("SET search_path={}", f.schema)).await;
            let ghost = format!("recipient_absent_{}", Uuid::new_v4().simple());
            let absent: bool = query("SELECT to_regnamespace($1) IS NULL AS yes")
                .bind(&ghost)
                .fetch_one(&f.admin)
                .await
                .unwrap()
                .get("yes");
            assert!(absent);
            let before = f.snapshot().await;
            // A nonexistent extra item must be rejected from RAW admission, even though PostgreSQL
            // ignores it and reports only the expected effective schema. This is not schema shadowing.
            configure(&f.pool, &format!("SET search_path={},{ghost}", f.schema)).await;
            let extra_get = f.store.get(grant(1), account(3)).await;
            let extra_issue = f
                .store
                .issue(
                    grant(1),
                    account(1),
                    file(1),
                    account(2),
                    account(3),
                    validity(),
                )
                .await;
            let extra_after = f.snapshot().await;
            configure(&f.pool, &format!("SET search_path={}", f.schema)).await;
            (extra_get, extra_issue, extra_after, before)
        })
    })
    .await;
    // Stop at this precise cleaned-up behavioral assertion for root's actual Red.
    assert_eq!(
        extra_get,
        Err(FileRecipientGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(
        extra_issue,
        Err(FileRecipientGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(extra_after, before);
    checked(ready(8).await, |f| {
        Box::pin(async move {
            let before = f.snapshot().await;
            configure(&f.pool, &format!("SET search_path={},pg_catalog", f.schema)).await;
            let result = f.store.get(grant(1), account(3)).await;
            assert_eq!(f.snapshot().await, before);
            assert_eq!(result, Err(FileRecipientGrantStoreError::UnsupportedSchema));
            configure(&f.pool, &format!("SET search_path={}", f.schema)).await;
            configure(
                &f.pool,
                &format!(
                    "CREATE TEMP TABLE {HEADS}(LIKE {}.{HEADS} INCLUDING ALL)",
                    f.schema
                ),
            )
            .await;
            let before = f.snapshot().await;
            let hidden = f.store.get(grant(1), account(3)).await;
            let failed = f
                .store
                .issue(
                    grant(1),
                    account(1),
                    file(1),
                    account(2),
                    account(3),
                    validity(),
                )
                .await;
            assert_eq!(f.snapshot().await, before);
            assert_eq!(hidden, Err(FileRecipientGrantStoreError::UnsupportedSchema));
            assert_eq!(failed, Err(FileRecipientGrantStoreError::UnsupportedSchema));
            // Dropping a TEMP table does not clear pg_my_temp_schema(): use healthy NEW backends.
            let (store, pool) = f.reopen().await;
            f.pool.close().await;
            f.pool = pool;
            f.store = store;
            let old = issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
            let mut holder = f.admin.begin().await.unwrap();
            let pid = backend(&mut holder).await;
            query(&format!(
                "LOCK TABLE {}.{MARKER} IN ROW EXCLUSIVE MODE",
                f.schema
            ))
            .execute(&mut *holder)
            .await
            .unwrap();
            query(&format!(
                "SELECT singleton FROM {}.{MARKER} FOR UPDATE",
                f.schema
            ))
            .fetch_one(&mut *holder)
            .await
            .unwrap();
            let mut pending = Box::pin(f.store.issue(
                grant(2),
                account(2),
                file(2),
                account(2),
                account(2),
                validity(),
            ));
            let mut early = None;
            let waited =
                tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(&f,pid)=>w};
            query(&format!(
                "UPDATE {}.{MARKER} SET installation_id=$1",
                f.schema
            ))
            .bind(Uuid::new_v4())
            .execute(&mut *holder)
            .await
            .unwrap();
            let injected = snapshot_in_tx(&f, &mut holder).await;
            holder.commit().await.unwrap();
            let result = match early {
                Some(r) => r,
                None => pending.as_mut().await,
            };
            drop(pending);
            assert_eq!(f.snapshot().await, injected);
            query(&format!(
                "UPDATE {}.{MARKER} SET installation_id=$1",
                f.schema
            ))
            .bind(f.installation_id)
            .execute(&f.pool)
            .await
            .unwrap();
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(old));
            issue(&f, 2, account(2), account(2), account(2), validity())
                .await
                .unwrap();
            assert!(waited);
            assert_eq!(
                result,
                Err(FileRecipientGrantStoreError::IncarnationMismatch)
            );
        })
    })
    .await;
}
