use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_namespace_installation_scope_limits_and_shapes_are_not_adopted() {
    let f = Fixture::new(8).await;
    sql(&f, "CREATE TABLE retained(value integer NOT NULL)").await;
    sql(&f, "INSERT INTO retained VALUES(17)").await;
    let before = snapshot(&f).await;
    let partial = f.store.install_empty_namespace().await;
    assert_eq!(snapshot(&f).await, before);
    f.cleanup().await;
    assert_eq!(
        partial,
        Err(FilePublisherGrantStoreError::NamespaceNotEmpty)
    );

    let f = ready(8).await;
    let before = snapshot(&f).await;
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(FilePublisherGrantStoreError::NamespaceNotEmpty)
    );
    assert_eq!(snapshot(&f).await, before);
    let wrong = FilePublisherGrantStore::new(f.pool.clone(), scope(), Uuid::new_v4(), &f.schema, 8)
        .unwrap();
    assert_eq!(
        wrong.get(grant(1), account(1)).await,
        Err(FilePublisherGrantStoreError::IncarnationMismatch)
    );
    let wrong =
        FilePublisherGrantStore::new(f.pool.clone(), scope(), f.installation_id, &f.schema, 9)
            .unwrap();
    assert_eq!(
        wrong.get(grant(1), account(1)).await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    for foreign in [
        WorkspaceRef {
            workspace_id: Uuid::from_u128(9001).try_into().unwrap(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: Uuid::from_u128(9002).try_into().unwrap(),
            ..scope()
        },
    ] {
        let wrong =
            FilePublisherGrantStore::new(f.pool.clone(), foreign, f.installation_id, &f.schema, 8)
                .unwrap();
        let subject = FileAccountRef::new(
            PrincipalRef {
                authority_id: foreign.authority_id,
                ..account(1).principal()
            },
            account(1).account_id(),
        );
        assert_eq!(
            wrong.get(grant(1), subject).await,
            Err(FilePublisherGrantStoreError::ScopeMismatch)
        );
    }
    assert_eq!(snapshot(&f).await, before);
    for column in ["namespace_oid", "schema_oid", "grants_oid", "changes_oid"] {
        sql(&f, &format!("UPDATE {MARKER} SET {column}={column}+1")).await;
        assert_eq!(
            get(&f, 1, account(1)).await,
            Err(FilePublisherGrantStoreError::UnsupportedSchema)
        );
        assert_eq!(
            issue(&f, 1, account(1), account(1), validity()).await,
            Err(FilePublisherGrantStoreError::UnsupportedSchema)
        );
        sql(&f, &format!("UPDATE {MARKER} SET {column}={column}-1")).await;
        assert!(get(&f, 1, account(1)).await.unwrap().is_none());
    }
    sql(
        &f,
        &format!("ALTER TABLE {HEADS} ALTER COLUMN subject_id DROP NOT NULL"),
    )
    .await;
    assert_eq!(
        get(&f, 1, account(1)).await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    sql(
        &f,
        &format!("ALTER TABLE {HEADS} ALTER COLUMN subject_id SET NOT NULL"),
    )
    .await;
    sql(
        &f,
        &format!("ALTER TABLE {CHANGES} DROP COLUMN recorded_at"),
    )
    .await;
    assert_eq!(
        get(&f, 1, account(1)).await,
        Err(FilePublisherGrantStoreError::UnsupportedSchema)
    );
    sql(&f,&format!("ALTER TABLE {CHANGES} ADD recorded_at BIGINT NOT NULL CHECK(recorded_at BETWEEN 0 AND 253402300799)")).await;
    issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    f.cleanup().await;
}
