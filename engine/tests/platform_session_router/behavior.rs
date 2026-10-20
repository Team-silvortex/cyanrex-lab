use super::fixture::*;
use axum::http::StatusCode;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_session_router_confirmed_login_query_logout_share_original_binding() {
    let (
        status,
        issued,
        authenticated,
        preserved,
        logged_out,
        revoked,
        acl,
        count,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let app = prepared.app().await;
            let response = capture(app.clone(), valid_request()).await;
            let status = response.status;
            let token = issued_token(&response);
            let mut authenticated = false;
            let mut preserved = false;
            let mut logged_out = false;
            let mut revoked = false;
            if let Some(token) = token.as_deref() {
                authenticated = current(&capture(app.clone(), get(token)).await);
                let before = snapshot(f).await;
                let mut req = valid_request();
                let mut cookie =
                    axum::http::HeaderValue::from_str(&format!("{COOKIE}={token}")).unwrap();
                cookie.set_sensitive(true);
                req.headers_mut().insert("cookie", cookie);
                let response = capture(app.clone(), req).await;
                preserved = denied(
                    &response,
                    StatusCode::CONFLICT,
                    "session_present",
                    "not_attempted",
                ) && before == snapshot(f).await;
                logged_out = cleared(&capture(app.clone(), logout(token)).await);
                let response = capture(app.clone(), get(token)).await;
                revoked = denied(
                    &response,
                    StatusCode::UNAUTHORIZED,
                    "invalid_session",
                    "unconfirmed",
                );
            }
            let issued = token.is_some() && watermark(f).await >= 0;
            drop(token);
            let count = session_count(f).await;
            let acl = prepared.minimal_acl().await;
            drop(response);
            drop(app);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                status,
                issued,
                authenticated,
                preserved,
                logged_out,
                revoked,
                acl,
                count,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        issued
            && authenticated
            && preserved
            && logged_out
            && revoked
            && acl
            && no_backend
            && cleaned
    );
    assert_eq!(count, 0);
}
