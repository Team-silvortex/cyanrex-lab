use super::fixture::*;
use axum::http::StatusCode;
use serde_json::json;
use sqlx_core::{query::query, row::Row};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_held_response_and_cancelled_actual_logout_wait_preserve_independent_results(
) {
    let (
        first,
        exhausted,
        healthy,
        before,
        after,
        cancelled,
        hit,
        waited,
        valid,
        restored,
        recovered,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let app = f.app_with_capacity(1);
            let before = f.snapshot().await;
            let held = within(app.clone().oneshot(request("GET", SESSION, &f.token)))
                .await
                .unwrap_or_else(|_| panic!("successful response admission failed"));
            // The first body remains unpolled while an otherwise-valid second GET is rejected.
            let exhausted = capture(app.clone(), request("GET", SESSION, &f.other_token)).await;
            let first = capture_response(held).await;
            let healthy = capture(app.clone(), request("GET", SESSION, &f.other_token)).await;

            f.fault(
                "BEFORE DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 74342); RETURN OLD;",
            )
            .await;
            let mut holder = within(f.pool().begin()).await.unwrap();
            within(
                query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 74342)")
                    .execute(&mut *holder),
            )
            .await
            .unwrap();
            let blocker: i32 =
                within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut *holder))
                    .await
                    .unwrap()
                    .get("pid");
            let req = request("POST", LOGOUT, &f.token);
            let waiting_app = app.clone();
            let pending = Pending::start(async move { capture(waiting_app, req).await });
            let waited = f.blocked_by(blocker).await;
            let cancelled = pending.cancel().await;
            within(holder.rollback()).await.unwrap();
            // Aborting the waiter is not rollback evidence. Drain the real SQL connection first.
            let restored = f.drain().await;
            let hit = f.calls().await;
            let after = f.snapshot().await;
            let valid = within(f.source().validate_session(&f.token))
                .await
                .unwrap_or_else(|_| panic!("cancelled logout control failed"))
                .is_some();
            let recovered = capture(app, request("GET", SESSION, &f.token)).await;
            (
                first, exhausted, healthy, before, after, cancelled, hit, waited, valid, restored,
                recovered,
            )
        })
    })
    .await;
    successful(
        &first,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    denied(
        &exhausted,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    successful(
        &healthy,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    successful(
        &recovered,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    assert!(
        cancelled && waited > 0 && valid && restored,
        "actual cancellation did not release its HTTP wait without inventing logout"
    );
    assert_eq!(hit, (true, 1));
    assert!(
        unchanged(&before, &after),
        "cancelled logout changed committed Source state"
    );
}
