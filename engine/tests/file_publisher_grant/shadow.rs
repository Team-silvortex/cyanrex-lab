use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_shadow_namespaces_and_marker_wait_drift_are_rejected() {
    let mut f = ready(8).await;
    let before = snapshot(&f).await;
    configure(&f.pool, &format!("SET search_path={},pg_catalog", f.schema)).await;
    assert_eq!(
        f.store.get(grant(1), account(1)).await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store
            .issue(grant(1), account(1), account(1), validity())
            .await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(snapshot(&f).await, before);
    configure(&f.pool, &format!("SET search_path={}", f.schema)).await;
    assert!(get(&f, 1, account(1)).await.unwrap().is_none());
    configure(
        &f.pool,
        &format!(
            "CREATE TEMP TABLE {HEADS}(LIKE {}.{HEADS} INCLUDING ALL)",
            f.schema
        ),
    )
    .await;
    let before = snapshot(&f).await;
    assert_eq!(
        f.store.get(grant(1), account(1)).await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store
            .issue(grant(1), account(1), account(1), validity())
            .await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    assert_eq!(snapshot(&f).await, before);
    // DROP TEMP TABLE cannot undo pg_my_temp_schema(). Close poisoned backends and open
    // fresh sessions with the same exact installation, never relax production admission.
    let (store, pool) = f.reopen().await;
    f.pool.close().await;
    f.pool = pool;
    f.store = store;
    let old = issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    let mut holder = f.admin.begin().await.unwrap();
    let pid = backend(&mut holder).await;
    // Admit this writer's table lock before the store's stronger fence. Otherwise
    // upgrading FOR UPDATE's ROW SHARE to ROW EXCLUSIVE behind SRE would deadlock.
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
    let mut pending = Box::pin(f.store.issue(grant(2), account(2), account(2), validity()));
    let waited = tokio::select! {_=&mut pending=>false,w=blocked(&f,pid,1)=>w};
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
    let result = pending.await;
    assert_eq!(snapshot(&f).await, injected);
    query(&format!(
        "UPDATE {}.{MARKER} SET installation_id=$1",
        f.schema
    ))
    .bind(f.installation_id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert_eq!(get(&f, 1, account(1)).await.unwrap(), Some(old));
    issue(&f, 2, account(2), account(2), validity())
        .await
        .unwrap();
    f.cleanup().await;
    assert!(waited);
    assert_eq!(
        result,
        Err(FilePublisherGrantStoreError::IncarnationMismatch)
    );
}
