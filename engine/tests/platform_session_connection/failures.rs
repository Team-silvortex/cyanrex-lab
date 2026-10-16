use super::{
    catalog_fixture::Fixture,
    fixture::{close_runtime, Prepared},
    http_helpers::{capture, denied, request, LOGOUT},
    marker_fixture::snapshot,
    storage_helpers::no_connections,
};
use axum::http::StatusCode;
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_rejects_readonly_and_missing_runtime_permissions_without_cookie_clear(
) {
    let (reads, failures, recovered, unchanged, no_backend, cleaned) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let prepared = Prepared::new(fixture).await;
                let connector = prepared.connector().await;
                let app = prepared.app().await;
                let before = snapshot(fixture).await;
                let mut reads = Vec::new();
                let mut failures = Vec::new();
                let mut unchanged = true;
                fixture
                    .sql(&format!(
                        "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on",
                        fixture.role, fixture.database
                    ))
                    .await;
                close_runtime(fixture).await;
                reads.push(
                    connector
                        .validate_session(&prepared.token)
                        .await
                        .map(|s| s.is_some()),
                );
                failures.push(capture(app.clone(), request("POST", LOGOUT, &prepared.token)).await);
                unchanged &= before == snapshot(fixture).await;
                fixture
                    .sql(&format!(
                        "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",
                        fixture.role, fixture.database
                    ))
                    .await;
                close_runtime(fixture).await;
                let marker = format!("{}.collaboration_auth_source_schema", fixture.source);
                fixture
                    .sql(&format!(
                        "REVOKE UPDATE(singleton) ON {marker} FROM {}",
                        fixture.role
                    ))
                    .await;
                reads.push(
                    connector
                        .validate_session(&prepared.token)
                        .await
                        .map(|s| s.is_some()),
                );
                failures.push(capture(app.clone(), request("POST", LOGOUT, &prepared.token)).await);
                unchanged &= before == snapshot(fixture).await;
                fixture
                    .sql(&format!(
                        "GRANT UPDATE(singleton) ON {marker} TO {}",
                        fixture.role
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "REVOKE DELETE ON {}.sessions FROM {}",
                        fixture.source, fixture.role
                    ))
                    .await;
                failures.push(capture(app, request("POST", LOGOUT, &prepared.token)).await);
                unchanged &= before == snapshot(fixture).await;
                fixture
                    .sql(&format!(
                        "GRANT DELETE ON {}.sessions TO {}",
                        fixture.source, fixture.role
                    ))
                    .await;
                let recovered = connector
                    .validate_session(&prepared.token)
                    .await
                    .map(|s| s.is_some());
                drop(connector);
                unchanged &= before == snapshot(fixture).await && prepared.minimal_acl().await;
                let no_backend = no_connections(fixture).await;
                let cleaned = prepared.finish();
                (reads, failures, recovered, unchanged, no_backend, cleaned)
            })
        })
        .await;
    assert_eq!(reads, vec![Err(DurableAuthError::StorageUnavailable); 2]);
    assert_eq!(failures.len(), 3);
    for response in &failures {
        denied(
            response,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
    }
    assert_eq!(recovered, Ok(true));
    assert!(unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_preserves_original_marker_and_root_bindings() {
    let (root_rejected,marker_rejected,responses,control,recovered,unchanged,no_backend,cleaned)=Fixture::checked(|fixture| {
        Box::pin(async move {
            let prepared=Prepared::new(fixture).await;
            let connector=prepared.connector().await;
            let app=prepared.app().await;
            let before=snapshot(fixture).await;
            let swapped=prepared.root().swap();
            let root_rejected=swapped && connector.validate_session(&prepared.token).await==Err(DurableAuthError::StorageUnavailable);
            let mut responses=vec![capture(app.clone(),request("POST",LOGOUT,&prepared.token)).await];
            let restored=prepared.root().restore();
            let mut unchanged=restored && before==snapshot(fixture).await;
            let marker=format!("{}.collaboration_auth_source_schema",fixture.source);
            fixture.sql(&format!("ALTER TABLE {marker} RENAME TO platform_session_saved_marker")).await;
            fixture.sql(&format!("CREATE TABLE {marker} (singleton boolean NOT NULL PRIMARY KEY,version integer NOT NULL,authority_id uuid NOT NULL)")).await;
            fixture.sql(&format!("INSERT INTO {marker} VALUES(TRUE,2,'{}')",fixture.config().scope().authority_id.as_uuid())).await;
            fixture.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}",fixture.role)).await;
            let replacement=snapshot(fixture).await;
            // Without the original-storage binding this otherwise valid Source is accepted.
            let pooled=cyanrex_engine::services::auth_service::durable_source::DurableAuthSource::new(
                fixture.runtime().clone(),fixture.config().scope().authority_id);
            let control=pooled.validate_session(&prepared.token).await.map(|s|s.is_some());
            let marker_rejected=connector.validate_session(&prepared.token).await;
            responses.push(capture(app,request("POST",LOGOUT,&prepared.token)).await);
            unchanged &= replacement==snapshot(fixture).await;
            fixture.sql(&format!("DROP TABLE {marker}")).await;
            fixture.sql(&format!("ALTER TABLE {}.platform_session_saved_marker RENAME TO collaboration_auth_source_schema",fixture.source)).await;
            let recovered=connector.validate_session(&prepared.token).await.map(|s|s.is_some());
            drop(pooled);drop(connector);
            unchanged &= before==snapshot(fixture).await;
            let no_backend=no_connections(fixture).await;
            let cleaned=prepared.finish();
            (root_rejected,marker_rejected,responses,control,recovered,unchanged,no_backend,cleaned)
        })
    }).await;
    assert!(root_rejected);
    assert_eq!(marker_rejected, Err(DurableAuthError::StorageUnavailable));
    assert_eq!(control, Ok(true));
    assert_eq!(recovered, Ok(true));
    for response in &responses {
        denied(
            response,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
    }
    assert!(unchanged && no_backend && cleaned);
}
