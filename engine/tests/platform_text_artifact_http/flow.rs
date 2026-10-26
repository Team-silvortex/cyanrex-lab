use super::fixture::*;
use axum::{body::HttpBody, http::StatusCode, response::Response};
use http_body_util::BodyExt;
use serde_json::Value;
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
    f.sql(&format!("CREATE FUNCTION {0}.text_http_pause() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('{0}'),932); RETURN NEW; END $$",f.artifacts)).await;
    f.sql(&format!("CREATE CONSTRAINT TRIGGER text_http_pause AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.text_http_pause()",f.artifacts)).await;
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query("SELECT pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext($1),932)")
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
    tokio::time::timeout(Duration::from_secs(3),async {loop {
        let row=within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))").bind(&f.database).bind(&f.role).bind(blocker).fetch_optional(f.owner())).await.unwrap();
        if row.is_some(){break;}tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await.expect("must observe the actual retained SQL connection waiting on our deferred fence");
}
async fn release(mut connection: PgConnection) {
    within(query("ROLLBACK").execute(&mut connection))
        .await
        .unwrap();
    within(connection.close()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_http_observed_sql_wait_expiry_cancel_and_response_capacity(
) {
    for mode in ["expiry", "cancel"] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;let route=router(&p,1).await;
            let files=p.files();let (fence,pid)=holder(f).await;
            let expiry=if mode=="expiry" {Some(within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at",f.source)).bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get::<chrono::DateTime<chrono::Utc>,_>("expires_at"))}else{None};
            let before=snapshot(f).await;
            let mut pending=Pending(tokio::spawn(route.clone().oneshot(request(&p,0,"POST",PATH,wire(1,"Observed real SQL wait","Unconfirmed waiting bytes")))));
            blocked(f,pid).await;assert!(!pending.0.is_finished());p.old_files_unchanged(&files,1);
            let unpublished=p.blob(&p.reference(1,b"Unconfirmed waiting bytes"));assert_eq!(std::fs::read(&unpublished).unwrap(),b"Unconfirmed waiting bytes");
            error(&route,request(&p,0,"POST",PATH,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
            if let Some(expiry)=expiry {
                tokio::time::timeout(Duration::from_secs(2),async {loop {
                    let expired:bool=within(query("SELECT clock_timestamp()>=$1 AS expired").bind(expiry).fetch_one(f.owner())).await.unwrap().get("expired");
                    if expired{break;}tokio::time::sleep(Duration::from_millis(5)).await;
                }}).await.unwrap();
                assert!(!pending.0.is_finished());release(fence).await;
                let response=(&mut pending.0).await.unwrap().unwrap();
                assert_eq!(value(response,StatusCode::UNAUTHORIZED).await,serde_json::json!({"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}));
            } else {
                pending.0.abort();assert!((&mut pending.0).await.unwrap_err().is_cancelled());release(fence).await;
            }
            assert!(super::storage_helpers::no_connections(f).await);assert_eq!(before,snapshot(f).await);p.counts(0).await;p.old_files_unchanged(&files,1);
            assert_eq!(std::fs::read(unpublished).unwrap(),b"Unconfirmed waiting bytes");
            // The controlled cancellation occurred at a known pre-commit SQL fence. It does
            // not establish what happens when a real network loses a COMMIT acknowledgement.
            if mode=="cancel" {
                f.sql(&format!("DROP TRIGGER text_http_pause ON {}.collaboration_artifact_outbox",f.artifacts)).await;
                let response=within(route.clone().oneshot(request(&p,0,"POST",PATH,wire(2,"Unread response","Committed frame")))).await.unwrap();
                assert_eq!(response.status(),StatusCode::CREATED);headers(&response);
                error(&route,request(&p,0,"POST",PATH,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
                let mut body=response.into_body();
                let bytes=within(body.frame()).await.unwrap().unwrap().into_data().unwrap();let created:Value=serde_json::from_slice(&bytes).unwrap();
                assert_eq!(created["artifact"]["reference"]["artifact_id"],serde_json::json!(artifact_id(2)));
                assert!(body.is_end_stream(),"last JSON frame releases admission even while Body remains owned");
                let response=within(route.clone().oneshot(request(&p,0,"POST",PATH,wire(3,"Dropped response","Also committed")))).await.unwrap();
                assert_eq!(response.status(),StatusCode::CREATED);headers(&response);
                error(&route,request(&p,0,"POST",PATH,unread()),StatusCode::SERVICE_UNAVAILABLE,"capacity_exhausted","not_attempted").await;
                drop(response);
                error(&route,request(&p,0,"POST",PATH,wire(3,"Dropped response","Also committed")),StatusCode::CONFLICT,"conflict","unconfirmed").await;
                p.counts(2).await;p.old_files_unchanged(&files,3);drop(body);
            }
            drop((route,pending));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
