use super::{
    catalog_fixture::Fixture,
    fixture::{otp, password, read_only, session_count, snapshot, username, watermark, Prepared},
    storage_helpers::no_connections,
};
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_missing_credential_columns_preserve_every_row() {
    let (failures, unchanged, acl, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let mut failures = Vec::new();
            let mut unchanged = true;
            for col in [
                "password_salt",
                "password_hash",
                "totp_secret",
                "otp_last_counter",
            ] {
                let connector = prepared.connector().await;
                f.sql(&format!(
                    "REVOKE SELECT ({col}) ON {}.users FROM {}",
                    f.source, f.role
                ))
                .await;
                let before = snapshot(f).await;
                failures.push(
                    connector
                        .login(&username(), password(), &otp())
                        .await
                        .map(|_| ()),
                );
                unchanged &= before == snapshot(f).await;
                f.sql(&format!(
                    "GRANT SELECT ({col}) ON {}.users TO {}",
                    f.source, f.role
                ))
                .await;
            }
            let acl = prepared.minimal_acl().await;
            unchanged &= watermark(f).await == -1 && session_count(f).await == 0;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (failures, unchanged, acl, no_backend, cleaned)
        })
    })
    .await;
    assert_eq!(failures, vec![Err(DurableAuthError::StorageUnavailable); 4]);
    assert!(unchanged && acl && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_missing_issuer_writes_roll_back_otp_and_session() {
    let (failures, unchanged, acl, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let mut failures = Vec::new();
            let mut unchanged = true;
            for (table, permission) in [
                ("sessions", "INSERT (token,username,account_id,expires_at)"),
                ("users", "UPDATE (otp_last_counter)"),
                ("collaboration_auth_source_schema", "UPDATE (singleton)"),
                ("sessions", "UPDATE (token)"),
            ] {
                let connector = prepared.connector().await;
                f.sql(&format!(
                    "REVOKE {permission} ON {}.{table} FROM {}",
                    f.source, f.role
                ))
                .await;
                let before = snapshot(f).await;
                failures.push(
                    connector
                        .login(&username(), password(), &otp())
                        .await
                        .map(|_| ()),
                );
                unchanged &= before == snapshot(f).await
                    && watermark(f).await == -1
                    && session_count(f).await == 0;
                f.sql(&format!(
                    "GRANT {permission} ON {}.{table} TO {}",
                    f.source, f.role
                ))
                .await;
            }
            let acl = prepared.minimal_acl().await;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (failures, unchanged, acl, no_backend, cleaned)
        })
    })
    .await;
    assert_eq!(failures, vec![Err(DurableAuthError::StorageUnavailable); 4]);
    assert!(unchanged && acl && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_explicit_readonly_policy_is_not_overridden() {
    let (failed, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let connector = prepared.connector().await;
            read_only(f, true).await;
            let before = snapshot(f).await;
            let failed = connector
                .login(&username(), password(), &otp())
                .await
                .map(|_| ());
            let unchanged = before == snapshot(f).await
                && watermark(f).await == -1
                && session_count(f).await == 0;
            read_only(f, false).await;
            drop(connector);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (failed, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert_eq!(failed, Err(DurableAuthError::StorageUnavailable));
    assert!(unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_retains_original_root_and_marker_identity() {
    let (root,marker,unchanged,recovered,no_backend,cleaned)=Fixture::checked(|f|Box::pin(async move {
        let prepared=Prepared::new(f).await;
        let connector=prepared.connector().await;
        let before=snapshot(f).await;
        let swapped=prepared.root().swap();
        let root=swapped && matches!(connector.login(&username(),password(),&otp()).await,Err(DurableAuthError::StorageUnavailable));
        let restored=prepared.root().restore();
        let mut unchanged=restored && before==snapshot(f).await;
        let relation=format!("{}.collaboration_auth_source_schema",f.source);
        f.sql(&format!("ALTER TABLE {relation} RENAME TO platform_login_original_marker")).await;
        f.sql(&format!("CREATE TABLE {relation} (singleton boolean PRIMARY KEY CHECK(singleton),version integer NOT NULL,authority_id uuid NOT NULL)")).await;
        f.sql(&format!("INSERT INTO {relation} VALUES(TRUE,2,'{}')",f.config().scope().authority_id.as_uuid())).await;
        f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {relation} TO {}",f.role)).await;
        let replacement=snapshot(f).await;
        let marker=connector.login(&username(),password(),&otp()).await.map(|_|());
        unchanged &= replacement==snapshot(f).await && watermark(f).await== -1 && session_count(f).await==0;
        f.sql(&format!("DROP TABLE {relation}")).await;
        f.sql(&format!("ALTER TABLE {}.platform_login_original_marker RENAME TO collaboration_auth_source_schema",f.source)).await;
        unchanged &= before==snapshot(f).await;
        // A new explicit attempt, not an automatic retry after an uncertain result.
        let recovered=connector.login(&username(),password(),&otp()).await.map(|_|());
        drop(connector);
        let no_backend=no_connections(f).await;
        let cleaned=prepared.finish();
        (root,marker,unchanged,recovered,no_backend,cleaned)
    })).await;
    assert!(root);
    assert_eq!(marker, Err(DurableAuthError::StorageUnavailable));
    assert_eq!(recovered, Ok(()));
    assert!(unchanged && no_backend && cleaned);
}
