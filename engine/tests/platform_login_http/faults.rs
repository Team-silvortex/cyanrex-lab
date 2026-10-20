use super::fixture::*;
use axum::http::{header, HeaderValue, StatusCode};

#[derive(Clone, Copy)]
enum Fault {
    CredentialRead,
    SessionInsert,
    ReadOnly,
    Root,
    Marker,
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_permissions_readonly_and_original_binding_fail_privately() {
    for fault in [
        Fault::CredentialRead,
        Fault::SessionInsert,
        Fault::ReadOnly,
        Fault::Root,
        Fault::Marker,
    ] {
        let (refused, unchanged, restored, acl, no_backend, cleaned) = Fixture::checked(|f| Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let app = app(&prepared).await;
            let before = snapshot(f).await;
            let relation = format!("{}.collaboration_task_schema", f.task);
            let changed = match fault {
                Fault::CredentialRead => {
                    f.sql(&format!("REVOKE SELECT(password_salt) ON {}.users FROM {}", f.source, f.role)).await;
                    true
                }
                Fault::SessionInsert => {
                    f.sql(&format!("REVOKE INSERT(token,username,account_id,expires_at) ON {}.sessions FROM {}", f.source, f.role)).await;
                    true
                }
                Fault::ReadOnly => { read_only(f, true).await; true }
                Fault::Root => prepared.root().swap(),
                Fault::Marker => {
                    f.sql(&format!("ALTER TABLE {relation} RENAME TO http_original_task_marker")).await;
                    f.sql(&format!("CREATE TABLE {relation} (LIKE {}.http_original_task_marker INCLUDING ALL)", f.task)).await;
                    f.sql(&format!("INSERT INTO {relation} SELECT * FROM {}.http_original_task_marker", f.task)).await;
                    f.sql(&format!("GRANT SELECT ON {relation} TO {}", f.role)).await;
                    true
                }
            };
            let drifted = snapshot(f).await;
            let mut req = valid_request();
            let mut legacy = HeaderValue::from_static("cyanrex_session=synthetic-teaching-cookie-preserved");
            legacy.set_sensitive(true);
            req.headers_mut().insert(header::COOKIE, legacy);
            let response = capture(app.clone(), req).await;
            let refused = denied(&response, StatusCode::SERVICE_UNAVAILABLE, "service_unavailable", "unconfirmed");
            let unchanged = changed && drifted == snapshot(f).await
                && session_count(f).await == 0 && watermark(f).await == -1;
            let restored = match fault {
                Fault::CredentialRead => {
                    f.sql(&format!("GRANT SELECT(password_salt) ON {}.users TO {}", f.source, f.role)).await;
                    true
                }
                Fault::SessionInsert => {
                    f.sql(&format!("GRANT INSERT(token,username,account_id,expires_at) ON {}.sessions TO {}", f.source, f.role)).await;
                    true
                }
                Fault::ReadOnly => { read_only(f, false).await; true }
                Fault::Root => prepared.root().restore(),
                Fault::Marker => {
                    f.sql(&format!("DROP TABLE {relation}")).await;
                    f.sql(&format!("ALTER TABLE {}.http_original_task_marker RENAME TO collaboration_task_schema", f.task)).await;
                    true
                }
            } && before == snapshot(f).await;
            let acl = prepared.minimal_acl().await;
            drop(response);
            drop(app);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (refused, unchanged, restored, acl, no_backend, cleaned)
        })).await;
        assert!(refused && unchanged && restored && acl && no_backend && cleaned);
    }
}
