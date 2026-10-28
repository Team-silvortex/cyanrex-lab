use super::fixture::*;
use axum::{
    body::{Body, Bytes, HttpBody},
    http::StatusCode,
    response::Response,
};
use cyanrex_engine::platform_http::{TaskContentHttpConfigError, TaskTextArtifactPublishHttpState};
use http_body_util::BodyExt;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;
use tower::ServiceExt;

struct Pending(tokio::task::JoinHandle<Result<Response, std::convert::Infallible>>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn holder(f: &Fixture) -> (PgConnection, i32) {
    f.sql(&format!("CREATE FUNCTION {0}.task_text_http_pause() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('{0}'),972); RETURN NEW; END $$", f.artifacts)).await;
    f.sql(&format!("CREATE CONSTRAINT TRIGGER task_text_http_pause AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.task_text_http_pause()", f.artifacts)).await;
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query("SELECT pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext($1),972)")
            .bind(&f.artifacts)
            .execute(&mut connection),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, pid)
}
async fn blocked(f: &Fixture, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(2), async { loop {
        let rows = within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND wait_event_type='Lock' AND $3=ANY(pg_catalog.pg_blocking_pids(pid)) AND pg_catalog.strpos(query,'SET CONSTRAINTS ALL IMMEDIATE')>0")
            .bind(&f.database).bind(&f.role).bind(blocker).fetch_all(f.owner())).await.unwrap();
        if !rows.is_empty() { assert_eq!(rows.len(), 1); break; } tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await.expect("must observe this exact retained SQL transaction at the pre-COMMIT fence");
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_response_capacity_body_deadline_and_cancel() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p.task(10, 0, "Response lifetime is not Task saving").await;
            for capacity in [0, 17] {
                assert!(matches!(
                    TaskTextArtifactPublishHttpState::with_capacity(p.connector().await, capacity),
                    Err(TaskContentHttpConfigError::InvalidCapacity)
                ));
            }
            let route = router(&p, 1).await;
            let before = snapshot(f).await;
            let files = p.files();
            let started = tokio::time::Instant::now();
            error(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    Body::from_stream(
                        futures_util::stream::pending::<Result<Bytes, std::io::Error>>(),
                    ),
                ),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                "not_attempted",
            )
            .await;
            assert!(started.elapsed() >= Duration::from_millis(1800));
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            let polled = std::sync::Arc::new(tokio::sync::Notify::new());
            let signal = polled.clone();
            let body = Body::from_stream(futures_util::stream::poll_fn(
                move |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
                    signal.notify_one();
                    std::task::Poll::Pending
                },
            ));
            let mut receiving = Pending(tokio::spawn(route.clone().oneshot(request(
                &p,
                0,
                "POST",
                &path(&old),
                body,
            ))));
            within(polled.notified()).await;
            error(
                &route,
                request(&p, 0, "POST", &path(&old), unread()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            receiving.0.abort();
            assert!((&mut receiving.0).await.unwrap_err().is_cancelled());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            let (mut fence, pid) = holder(f).await;
            let mut pending = Pending(tokio::spawn(route.clone().oneshot(request(
                &p,
                0,
                "POST",
                &path(&old),
                wire(&old, 1, "Unconfirmed waiting bytes"),
            ))));
            blocked(f, pid).await;
            assert!(!pending.0.is_finished());
            p.old_files_unchanged(&files, 1);
            error(
                &route,
                request(&p, 0, "POST", &path(&old), unread()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            pending.0.abort();
            assert!((&mut pending.0).await.unwrap_err().is_cancelled());
            within(query("ROLLBACK").execute(&mut fence)).await.unwrap();
            within(fence.close()).await.unwrap();
            assert!(super::storage_helpers::no_connections(f).await);
            assert_eq!(before, snapshot(f).await);
            p.counts(0).await;
            p.old_files_unchanged(&files, 1);
            assert_eq!(
                std::fs::read(p.blob(&p.reference(1, b"Unconfirmed waiting bytes"))).unwrap(),
                b"Unconfirmed waiting bytes"
            );
            f.sql(&format!(
                "DROP TRIGGER task_text_http_pause ON {}.collaboration_artifact_outbox",
                f.artifacts
            ))
            .await;
            let response = within(route.clone().oneshot(request(
                &p,
                0,
                "POST",
                &path(&old),
                wire(&old, 2, "Unread committed response"),
            )))
            .await
            .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            headers(&response);
            error(
                &route,
                request(&p, 0, "POST", &path(&old), unread()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            let mut body = response.into_body();
            let bytes = within(body.frame())
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap();
            let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                reply["artifact"]["reference"]["artifact_id"],
                serde_json::json!(artifact_id(2))
            );
            assert!(
                body.is_end_stream(),
                "last JSON frame releases the permit while Body remains owned"
            );
            let response = within(route.clone().oneshot(request(
                &p,
                0,
                "POST",
                &path(&old),
                wire(&old, 3, "Dropped but committed response"),
            )))
            .await
            .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
            error(
                &route,
                request(&p, 0, "POST", &path(&old), unread()),
                StatusCode::SERVICE_UNAVAILABLE,
                "capacity_exhausted",
                "not_attempted",
            )
            .await;
            drop(response);
            error(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    wire(&old, 3, "Dropped but committed response"),
                ),
                StatusCode::CONFLICT,
                "conflict",
                "unconfirmed",
            )
            .await;
            p.counts(2).await;
            p.old_files_unchanged(&files, 3);
            p.unchanged(&old).await;
            assert_eq!(
                before
                    .into_iter()
                    .filter(|(table, _)| !table.starts_with(&format!("{}.", f.artifacts)))
                    .collect::<Vec<_>>(),
                p.non_artifacts().await
            );
            drop((route, body, pending, receiving));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
