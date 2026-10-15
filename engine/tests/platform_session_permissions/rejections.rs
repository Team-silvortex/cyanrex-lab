use super::{fixture::Restricted, http_fixture::*};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_permissions_missing_select_update_or_delete_never_mutates_or_clears_cookie(
) {
    let results = Fixture::checked(|f| Box::pin(async move {
        Restricted::checked(f, |r| Box::pin(async move {
            let mut results = Vec::new();
            for (privilege, method, path) in [
                ("SELECT (username, account_id) ON users", "GET", SESSION),
                ("SELECT (token, username, account_id, expires_at) ON sessions", "GET", SESSION),
                ("SELECT (singleton, version, authority_id) ON collaboration_auth_source_schema", "GET", SESSION),
                ("UPDATE (username) ON users", "GET", SESSION),
                ("UPDATE (token) ON sessions", "GET", SESSION),
                ("UPDATE (singleton) ON collaboration_auth_source_schema", "GET", SESSION),
                ("DELETE ON sessions", "POST", LOGOUT),
            ] {
                r.admin_sql(&format!("REVOKE {privilege} FROM {}", r.role)).await;
                let before = r.owner.snapshot().await;
                let reply = capture(r.app(), request(method, path, &r.owner.token)).await;
                let after = r.owner.snapshot().await;
                r.admin_sql(&format!("GRANT {privilege} TO {}", r.role)).await;
                let restored = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                results.push((reply, before, after, restored, r.minimal_acl().await));
            }
            results
        })).await
    })).await;
    assert_eq!(results.len(), 7);
    for (reply, before, after, restored, acl) in results {
        denied(
            &reply,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
        assert!(
            unchanged(&before, &after),
            "missing privilege changed Source state"
        );
        assert!(acl, "permission fixture failed to restore exact grants");
        successful(
            &restored,
            json!({"schema_version":1,"authenticated":true}),
            false,
        );
    }
}
