use super::{
    catalog_fixture::{within, Fixture},
    fixture::{session_count, stable, Prepared},
    http_helpers::{capture, denied, request, token_hash, SESSION},
    storage_helpers::no_connections,
};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

async fn pending(fixture: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_millis(900), async {
        loop {
            let row = query(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity
                WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection'
                AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS pending",
            )
            .bind(&fixture.database)
            .bind(&fixture.role)
            .bind(blocker)
            .fetch_one(fixture.owner())
            .await
            .unwrap();
            if row.get::<bool, _>("pending") {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_uses_fresh_expiry_after_real_source_fence_wait() {
    let (result,waited,elapsed,released,response,other,remaining,unchanged,no_backend,cleaned)=Fixture::checked(|fixture| {
        Box::pin(async move {
            let prepared=Prepared::new(fixture).await;
            let connector=prepared.connector().await;
            let app=prepared.app().await;
            let before=stable(fixture).await;
            let mut holder=within(PgConnection::connect_with(fixture.owner().connect_options().as_ref())).await.unwrap();
            within(query("BEGIN").execute(&mut holder)).await.unwrap();
            within(query(&format!("SELECT singleton FROM {}.collaboration_auth_source_schema FOR UPDATE",fixture.source))
                .fetch_one(&mut holder)).await.unwrap();
            let pid=within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut holder)).await.unwrap().get("pid");
            let (result,(waited,elapsed,released))=tokio::join!(connector.validate_session(&prepared.token),async {
                let waited=pending(fixture,pid).await;
                // The business transaction demonstrably exists before this deadline.
                let expiry:DateTime<Utc>=within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '100 milliseconds'
                    WHERE token=$1 RETURNING expires_at",fixture.source)).bind(token_hash(&prepared.token))
                    .fetch_one(&mut holder)).await.unwrap().get("expires_at");
                let elapsed=tokio::time::timeout(Duration::from_millis(500),async {
                    loop {
                        let row=query("SELECT clock_timestamp()>=$1 AS elapsed").bind(expiry).fetch_one(fixture.owner()).await.unwrap();
                        if row.get::<bool,_>("elapsed"){return true;}
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                }).await.unwrap_or(false);
                let committed=within(query("COMMIT").execute(&mut holder)).await.is_ok();
                let closed=within(holder.close()).await.is_ok();
                (waited,elapsed,committed&&closed)
            });
            let result=result.map(|s|s.is_none());
            let response=capture(app,request("GET",SESSION,&prepared.token)).await;
            let other=connector.validate_session(&prepared.other).await.map(|s|s.is_some());
            drop(connector);
            let remaining=session_count(fixture).await;
            let unchanged=before==stable(fixture).await;
            let no_backend=no_connections(fixture).await;
            let cleaned=prepared.finish();
            (result,waited,elapsed,released,response,other,remaining,unchanged,no_backend,cleaned)
        })
    }).await;
    assert!(waited && elapsed && released);
    assert_eq!(result, Ok(true));
    assert_eq!(other, Ok(true));
    denied(
        &response,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "unconfirmed",
    );
    assert_eq!(remaining, 2);
    assert!(unchanged && no_backend && cleaned);
}

async fn source_holder(fixture: &Fixture) -> (PgConnection, i32) {
    let mut holder = within(PgConnection::connect_with(
        fixture.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut holder)).await.unwrap();
    within(
        query(&format!(
            "SELECT singleton FROM {}.collaboration_auth_source_schema FOR UPDATE",
            fixture.source
        ))
        .fetch_one(&mut holder),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut holder))
        .await
        .unwrap()
        .get("pid");
    (holder, pid)
}

async fn release_source(mut holder: PgConnection) -> bool {
    let released = within(query("ROLLBACK").execute(&mut holder)).await.is_ok();
    let closed = within(holder.close()).await.is_ok();
    released && closed
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_connection_rechecks_original_owner_after_source_wait_and_cancellation(
) {
    let (
        response,
        waited,
        released,
        rolled_back,
        recovered,
        cancel_wait,
        cancelled,
        cancel_released,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let auxiliary = fixture.additional_role().await;
            let owner = super::expectation_helpers::quote_identifier(&fixture.owner_name().await);
            let prepared = Prepared::new(fixture).await;
            let connector = prepared.connector().await;
            let app = prepared.app().await;
            let before = super::marker_fixture::snapshot(fixture).await;
            let (holder, pid) = source_holder(fixture).await;
            let (response, (waited, changed, released)) = tokio::join!(
                capture(
                    app.clone(),
                    request("POST", super::http_helpers::LOGOUT, &prepared.token)
                ),
                async {
                    let waited = pending(fixture, pid).await;
                    // SourcePin retains name/OID, but not namespace ownership. Only the
                    // binding's in-business-transaction original catalog check rejects this.
                    fixture
                        .sql(&format!(
                            "ALTER SCHEMA {} OWNER TO {auxiliary}",
                            fixture.source
                        ))
                        .await;
                    let changed = super::marker_fixture::snapshot(fixture).await;
                    let released = release_source(holder).await;
                    (waited, changed, released)
                }
            );
            let rolled_back = changed == super::marker_fixture::snapshot(fixture).await
                && session_count(fixture).await == 2;
            fixture
                .sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", fixture.source))
                .await;
            let recovered = connector
                .validate_session(&prepared.token)
                .await
                .map(|s| s.is_some());

            let (holder, pid) = source_holder(fixture).await;
            let token = prepared.token.clone();
            let job = super::http_helpers::Pending::start(async move {
                capture(app, request("POST", super::http_helpers::LOGOUT, &token)).await
            });
            let cancel_wait = pending(fixture, pid).await;
            let cancelled = job.cancel().await;
            let cancel_released = release_source(holder).await;
            drop(connector);
            let no_backend = no_connections(fixture).await;
            let unchanged = before == super::marker_fixture::snapshot(fixture).await;
            let cleaned = prepared.finish();
            (
                response,
                waited,
                released,
                rolled_back,
                recovered,
                cancel_wait,
                cancelled,
                cancel_released,
                unchanged,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    denied(
        &response,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
    assert!(waited && released && rolled_back);
    assert_eq!(recovered, Ok(true));
    assert!(cancel_wait && cancelled && cancel_released && unchanged && no_backend && cleaned);
}
