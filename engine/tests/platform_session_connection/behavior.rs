use super::{
    catalog_fixture::Fixture,
    fixture::{session_count, stable, Prepared},
    http_helpers::{capture, denied, request, successful, LOGOUT, SESSION},
    storage_helpers::no_connections,
};
use axum::http::StatusCode;
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_minimal_role_queries_and_logs_out_existing_sessions()
{
    let (
        valid,
        unknown,
        invalid,
        bad_logout,
        unknown_logout,
        first,
        missing,
        logout,
        revoked,
        other,
        acl,
        credentials,
        unchanged,
        remaining,
        no_backend,
        root_cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let prepared = Prepared::new(fixture).await;
            let before = stable(fixture).await;
            let acl = prepared.minimal_acl().await;
            let credentials = prepared.credentials_denied().await;
            let connector = prepared.connector().await;
            let app = prepared.app().await;
            let valid = connector
                .validate_session(&prepared.token)
                .await
                .map(|value| {
                    value.is_some_and(|s| {
                        s.account.authority_id == fixture.config().scope().authority_id
                            && s.account.username.as_str() == "marker-fixture"
                            && s.account.account_id.as_uuid().to_string()
                                == "00000000-0000-0000-0000-000000008211"
                    })
                });
            let unknown = connector
                .validate_session(&prepared.unknown)
                .await
                .map(|value| value.is_none());
            let invalid = connector
                .validate_session("not-a-token")
                .await
                .map(|value| value.is_none());
            let bad_logout = connector.logout("not-a-token").await;
            let unknown_logout = connector.logout(&prepared.unknown).await;
            let first = capture(app.clone(), request("GET", SESSION, &prepared.token)).await;
            let missing = capture(app.clone(), request("GET", SESSION, &prepared.unknown)).await;
            let logout = capture(app.clone(), request("POST", LOGOUT, &prepared.token)).await;
            let revoked = connector
                .validate_session(&prepared.token)
                .await
                .map(|value| value.is_none());
            let other = capture(app, request("GET", SESSION, &prepared.other)).await;
            drop(connector);
            let unchanged = before == stable(fixture).await;
            let remaining = session_count(fixture).await;
            let no_backend = no_connections(fixture).await;
            let root_cleaned = prepared.finish();
            (
                valid,
                unknown,
                invalid,
                bad_logout,
                unknown_logout,
                first,
                missing,
                logout,
                revoked,
                other,
                acl,
                credentials,
                unchanged,
                remaining,
                no_backend,
                root_cleaned,
            )
        })
    })
    .await;
    // Genuine role/column controls and all behavior assertions follow root/SQL cleanup.
    assert!(acl && credentials);
    assert_eq!(valid, Ok(true));
    assert_eq!(unknown, Ok(true));
    assert_eq!(invalid, Ok(true));
    assert_eq!(bad_logout, Err(DurableAuthError::InvalidInput));
    assert_eq!(unknown_logout, Ok(()));
    successful(
        &first,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    denied(
        &missing,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "unconfirmed",
    );
    successful(&logout, json!({"schema_version":1,"ok":true}), true);
    assert_eq!(revoked, Ok(true));
    successful(
        &other,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    assert_eq!(remaining, 1);
    assert!(unchanged && no_backend && root_cleaned);
}
