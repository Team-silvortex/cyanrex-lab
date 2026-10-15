use super::fixture::*;
use axum::http::StatusCode;
use sqlx_core::{query::query, row::Row};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_get_rechecks_expiry_after_an_observed_source_row_lock_wait() {
    let (reply, before, after, waited, expired, still_pending, restored) = Fixture::checked(|f| Box::pin(async move {
        within(query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '900 milliseconds' WHERE token = $1")
            .bind(token_hash(&f.token)).execute(f.pool())).await
            .unwrap_or_else(|_| panic!("short-lived Session setup failed"));
        let before = f.snapshot().await;
        let mut holder = within(f.pool().begin()).await.unwrap();
        within(query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
            .fetch_one(&mut *holder)).await.unwrap();
        let blocker: i32 = within(query("SELECT pg_backend_pid() AS pid")
            .fetch_one(&mut *holder)).await.unwrap().get("pid");
        let app = f.app();
        let req = request("GET", SESSION, &f.token);
        let pending = Pending::start(async move { capture(app, req).await });
        let waited = f.blocked_by(blocker).await;
        let expired = within(async {
            loop {
                let expired: bool = query(&format!("SELECT expires_at <= clock_timestamp() AS expired
                    FROM {}.sessions WHERE token = $1", f.schema))
                    .bind(token_hash(&f.token)).fetch_one(f.admin()).await
                    .unwrap_or_else(|_| panic!("actual database expiry observation failed"))
                    .get("expired");
                if expired { break true; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await;
        let still_pending = !pending.is_finished();
        within(holder.rollback()).await.unwrap();
        let reply = pending.finish().await;
        let restored = f.drain().await;
        let after = f.snapshot().await;
        (reply, before, after, waited, expired, still_pending, restored)
    })).await;
    denied(
        &reply,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "unconfirmed",
    );
    assert!(
        waited > 0 && expired && still_pending && restored,
        "GET did not cross the exact blocked expiry window"
    );
    assert!(
        unchanged(&before, &after),
        "a Session read cleaned or changed expired state"
    );
    assert_eq!(after.session_count, 2);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_actual_delete_lock_timeout_returns_unconfirmed_without_cookie_clear()
{
    let (reply, before, after, hit, waited, valid, restored) = Fixture::checked(|f| {
        Box::pin(async move {
            f.fault(
                "BEFORE DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 74341); RETURN OLD;",
            )
            .await;
            let before = f.snapshot().await;
            let mut holder = within(f.pool().begin()).await.unwrap();
            within(
                query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 74341)")
                    .execute(&mut *holder),
            )
            .await
            .unwrap();
            let blocker: i32 =
                within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut *holder))
                    .await
                    .unwrap()
                    .get("pid");
            let app = f.app();
            let req = request("POST", LOGOUT, &f.token);
            let pending = Pending::start(async move { capture(app, req).await });
            let waited = f.blocked_by(blocker).await;
            // Keep the real blocker held until the original Source's lock_timeout returns an error.
            let reply = pending.finish().await;
            within(holder.rollback()).await.unwrap();
            let hit = f.calls().await;
            let restored = f.drain().await;
            let after = f.snapshot().await;
            let valid = within(f.source().validate_session(&f.token))
                .await
                .unwrap_or_else(|_| panic!("timed-out logout control failed"))
                .is_some();
            (reply, before, after, hit, waited, valid, restored)
        })
    })
    .await;
    denied(
        &reply,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    );
    assert!(waited > 0 && valid && restored);
    assert_eq!(
        hit,
        (true, 1),
        "timeout did not reach the actual DELETE trigger"
    );
    assert!(
        unchanged(&before, &after),
        "timeout was incorrectly treated as confirmed logout"
    );
}
