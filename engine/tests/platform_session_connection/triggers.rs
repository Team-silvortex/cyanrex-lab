use super::{
    catalog_fixture::{within, Fixture},
    fixture::{session_count, Prepared},
    http_helpers::{capture, denied, request, LOGOUT},
    marker_fixture::snapshot,
    storage_helpers::no_connections,
};
use axum::http::StatusCode;
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_delete_path_redirection_rolls_back_without_clearing_cookie(
) {
    let (
        failed,
        response,
        hits,
        retained,
        recovered,
        other,
        remaining,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let prepared = Prepared::new(fixture).await;
            let connector = prepared.connector().await;
            let app = prepared.app().await;
            let source = &fixture.source;
            fixture.sql("CREATE SCHEMA platform_session_shadow").await;
            fixture
                .sql("CREATE TABLE platform_session_shadow.sessions(token text PRIMARY KEY)")
                .await;
            fixture
                .sql(&format!(
                    "GRANT USAGE ON SCHEMA platform_session_shadow TO {}",
                    fixture.role
                ))
                .await;
            fixture
                .sql(&format!(
                    "GRANT SELECT ON platform_session_shadow.sessions TO {}",
                    fixture.role
                ))
                .await;
            fixture
                .sql(&format!(
                    "CREATE SEQUENCE {source}.platform_session_delete_hits"
                ))
                .await;
            fixture
                .sql(&format!(
                    "CREATE FUNCTION {source}.platform_session_redirect_delete() RETURNS trigger
                LANGUAGE plpgsql SECURITY DEFINER AS $body$ BEGIN
                PERFORM pg_catalog.nextval('{source}.platform_session_delete_hits');
                PERFORM pg_catalog.set_config('search_path','platform_session_shadow',true);
                RETURN NULL; END $body$"
                ))
                .await;
            fixture
                .sql(&format!(
                    "CREATE TRIGGER platform_session_redirect BEFORE DELETE ON {source}.sessions
                FOR EACH ROW EXECUTE FUNCTION {source}.platform_session_redirect_delete()"
                ))
                .await;
            let before = snapshot(fixture).await;
            let failed = connector.logout(&prepared.token).await;
            let response = capture(app, request("POST", LOGOUT, &prepared.token)).await;
            let hits: i64 = within(
                query(&format!(
                    "SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits
                FROM {source}.platform_session_delete_hits"
                ))
                .fetch_one(fixture.owner()),
            )
            .await
            .unwrap()
            .get("hits");
            let unchanged = before == snapshot(fixture).await;
            let retained = connector
                .validate_session(&prepared.token)
                .await
                .map(|s| s.is_some());
            fixture
                .sql(&format!(
                    "DROP TRIGGER platform_session_redirect ON {source}.sessions"
                ))
                .await;
            fixture
                .sql(&format!(
                    "DROP FUNCTION {source}.platform_session_redirect_delete()"
                ))
                .await;
            fixture
                .sql(&format!(
                    "DROP SEQUENCE {source}.platform_session_delete_hits"
                ))
                .await;
            fixture
                .sql("DROP SCHEMA platform_session_shadow CASCADE")
                .await;
            let recovered = connector.logout(&prepared.token).await;
            let other = connector
                .validate_session(&prepared.other)
                .await
                .map(|s| s.is_some());
            drop(connector);
            let remaining = session_count(fixture).await;
            let no_backend = no_connections(fixture).await;
            let cleaned = prepared.finish();
            (
                failed, response, hits, retained, recovered, other, remaining, unchanged,
                no_backend, cleaned,
            )
        })
    })
    .await;
    assert_eq!(failed, Err(DurableAuthError::StorageUnavailable));
    denied(
        &response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
    assert_eq!(hits, 2);
    assert_eq!(retained, Ok(true));
    assert_eq!(recovered, Ok(()));
    assert_eq!(other, Ok(true));
    assert_eq!(remaining, 1);
    assert!(unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_flushes_deferred_delete_reinsertion_before_success() {
    let (failed,response,hits,retained,recovered,other,remaining,unchanged,no_backend,cleaned)=Fixture::checked(|fixture| {
        Box::pin(async move {
            let prepared=Prepared::new(fixture).await;
            let connector=prepared.connector().await;
            let app=prepared.app().await;
            let source=&fixture.source;
            fixture.sql(&format!("CREATE SEQUENCE {source}.platform_session_deferred_hits")).await;
            fixture.sql(&format!("CREATE FUNCTION {source}.platform_session_reinsert_delete() RETURNS trigger
                LANGUAGE plpgsql SECURITY DEFINER AS $body$ BEGIN
                PERFORM pg_catalog.nextval('{source}.platform_session_deferred_hits');
                INSERT INTO {source}.sessions(token,username,account_id,expires_at,created_at)
                    VALUES(OLD.token,OLD.username,OLD.account_id,OLD.expires_at,OLD.created_at);
                RETURN NULL; END $body$")).await;
            fixture.sql(&format!("CREATE CONSTRAINT TRIGGER platform_session_deferred_reinsert AFTER DELETE ON {source}.sessions
                DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {source}.platform_session_reinsert_delete()")).await;
            let before=snapshot(fixture).await;
            let failed=connector.logout(&prepared.token).await;
            let response=capture(app,request("POST",LOGOUT,&prepared.token)).await;
            let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits
                FROM {source}.platform_session_deferred_hits")).fetch_one(fixture.owner())).await.unwrap().get("hits");
            let unchanged=before==snapshot(fixture).await;
            let retained=connector.validate_session(&prepared.token).await.map(|s|s.is_some());
            fixture.sql(&format!("DROP TRIGGER platform_session_deferred_reinsert ON {source}.sessions")).await;
            fixture.sql(&format!("DROP FUNCTION {source}.platform_session_reinsert_delete()")).await;
            fixture.sql(&format!("DROP SEQUENCE {source}.platform_session_deferred_hits")).await;
            // The failure does not authorize an automatic retry. This is a distinct,
            // explicit operation only after the trusted fixture removed its fault.
            let recovered=connector.logout(&prepared.token).await;
            let other=connector.validate_session(&prepared.other).await.map(|s|s.is_some());
            drop(connector);
            let remaining=session_count(fixture).await;
            let no_backend=no_connections(fixture).await;
            let cleaned=prepared.finish();
            (failed,response,hits,retained,recovered,other,remaining,unchanged,no_backend,cleaned)
        })
    }).await;
    assert_eq!(failed, Err(DurableAuthError::StorageUnavailable));
    denied(
        &response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
    assert_eq!(hits, 2);
    assert_eq!(retained, Ok(true));
    assert_eq!(recovered, Ok(()));
    assert_eq!(other, Ok(true));
    assert_eq!(remaining, 1);
    assert!(unchanged && no_backend && cleaned);
}
