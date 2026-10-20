use super::fixture::*;
use axum::http::StatusCode;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_original_root_and_marker_drift_reject_both_lanes_without_cached_auth(
) {
    let (issued, failures, unchanged, restored, recovered, no_backend, cleaned) = Fixture::checked(|f|Box::pin(async move {
        let prepared = Prepared::new(f).await;
        let app = prepared.app().await;
        let token = issued_token(&capture(app.clone(), valid_request()).await);
        let issued = token.is_some();
        let token = token.unwrap_or_else(||uuid::Uuid::new_v4().to_string());
        let before = snapshot(f).await;
        let mut failures = true;
        let mut unchanged = true;
        let mut restored = true;
        for root_drift in [true,false] {
            if root_drift { failures &= prepared.root().swap(); }
            else {
                // A non-Source marker prevents Source-only session checks from masking an
                // independently adopted or weaker lifecycle binding after successful login.
                f.sql(&format!("ALTER TABLE {}.collaboration_task_schema RENAME TO router_original_marker",f.task)).await;
                f.sql(&format!("CREATE TABLE {}.collaboration_task_schema (LIKE {}.router_original_marker INCLUDING ALL)",f.task,f.task)).await;
                f.sql(&format!("INSERT INTO {}.collaboration_task_schema SELECT * FROM {}.router_original_marker",f.task,f.task)).await;
                f.sql(&format!("GRANT SELECT ON {}.collaboration_task_schema TO {}",f.task,f.role)).await;
            }
            let drifted = snapshot(f).await;
            for req in [valid_request(),get(&token),logout(&token)] {
                failures &= denied(&capture(app.clone(),req).await, StatusCode::SERVICE_UNAVAILABLE,"service_unavailable","unconfirmed");
            }
            unchanged &= drifted==snapshot(f).await && session_count(f).await==1;
            if root_drift { restored &= prepared.root().restore(); }
            else {
                f.sql(&format!("DROP TABLE {}.collaboration_task_schema",f.task)).await;
                f.sql(&format!("ALTER TABLE {}.router_original_marker RENAME TO collaboration_task_schema",f.task)).await;
            }
            restored &= before==snapshot(f).await;
        }
        let recovered = current(&capture(app.clone(),get(&token)).await);
        drop(app);
        let no_backend = no_connections(f).await;
        let cleaned = prepared.finish();
        (issued,failures,unchanged,restored,recovered,no_backend,cleaned)
    })).await;
    assert!(issued && failures && unchanged && restored && recovered && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_readonly_and_missing_delete_never_clear_unconfirmed_logout(
) {
    let (issued, failures, unchanged, healthy, acl, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let app = prepared.app().await;
            let token = issued_token(&capture(app.clone(), valid_request()).await);
            let issued = token.is_some();
            let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            f.sql(&format!(
                "REVOKE DELETE ON {}.sessions FROM {}",
                f.source, f.role
            ))
            .await;
            let before = snapshot(f).await;
            let mut failures = denied(
                &capture(app.clone(), logout(&token)).await,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            );
            let mut unchanged = before == snapshot(f).await && session_count(f).await == 1;
            let mut healthy = current(&capture(app.clone(), get(&token)).await);
            f.sql(&format!(
                "GRANT DELETE ON {}.sessions TO {}",
                f.source, f.role
            ))
            .await;
            read_only(f, true).await;
            let before = snapshot(f).await;
            for req in [valid_request(), get(&token), logout(&token)] {
                failures &= denied(
                    &capture(app.clone(), req).await,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                );
            }
            unchanged &= before == snapshot(f).await && session_count(f).await == 1;
            read_only(f, false).await;
            // A suppressed real DELETE is an unconfirmed result, not permission to clear or retry.
            f.sql(&format!(
                "CREATE SEQUENCE {}.router_suppress_hits",
                f.source
            ))
            .await;
            f.sql(&format!(
                "CREATE FUNCTION {}.router_suppress_delete() RETURNS trigger
            LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN
            PERFORM pg_catalog.nextval('{}.router_suppress_hits'); RETURN NULL; END $$",
                f.source, f.source
            ))
            .await;
            f.sql(&format!(
                "CREATE TRIGGER router_suppress BEFORE DELETE ON {}.sessions
            FOR EACH ROW EXECUTE FUNCTION {}.router_suppress_delete()",
                f.source, f.source
            ))
            .await;
            let before = snapshot(f).await;
            failures &= denied(
                &capture(app.clone(), logout(&token)).await,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            );
            let hit = within(
                query(&format!(
                    "SELECT is_called AND last_value=1 AS hit FROM {}.router_suppress_hits",
                    f.source
                ))
                .fetch_one(f.owner()),
            )
            .await
            .unwrap()
            .get::<bool, _>("hit");
            failures &= hit;
            unchanged &= before == snapshot(f).await && session_count(f).await == 1;
            f.sql(&format!(
                "DROP TRIGGER router_suppress ON {}.sessions",
                f.source
            ))
            .await;
            f.sql(&format!("DROP SEQUENCE {}.router_suppress_hits", f.source))
                .await;
            f.sql(&format!(
                "DROP FUNCTION {}.router_suppress_delete()",
                f.source
            ))
            .await;
            healthy &= current(&capture(app.clone(), get(&token)).await);
            // Canonical unknown TOKEN is different: confirmed absence may still clear its cookie.
            let unknown = uuid::Uuid::new_v4().to_string();
            healthy &= cleared(&capture(app.clone(), logout(&unknown)).await)
                && session_count(f).await == 1;
            let acl = prepared.minimal_acl().await;
            drop(app);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                issued, failures, unchanged, healthy, acl, no_backend, cleaned,
            )
        })
    })
    .await;
    assert!(issued && failures && unchanged && healthy && acl && no_backend && cleaned);
}
