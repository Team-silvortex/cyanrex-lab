use super::fixture::*;
use axum::http::{header, HeaderValue, StatusCode};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_confirmed_cookie_uses_restricted_issuer_and_stored_digest() {
    let (issued, exact, valid, acl, count, consumed, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let acl = prepared.minimal_acl().await;
            let app = app(&prepared).await;
            let mut req = valid_request();
            let mut legacy =
                HeaderValue::from_static("cyanrex_session=synthetic-old-teaching-session");
            legacy.set_sensitive(true);
            req.headers_mut().insert(header::COOKIE, legacy);
            let response = capture(app.clone(), req).await;
            let token = issued_token(&response);
            let mut exact = false;
            let mut valid = false;
            if let Some(token) = token.as_deref() {
                exact = stored_only_as_digest(f, token).await;
                let sessions = prepared.session_connector().await;
                valid = sessions.validate_session(token).await.is_ok_and(|value| {
                    value.is_some_and(|session| {
                        session.account.username == username()
                            && session.account.authority_id == f.config().scope().authority_id
                    })
                });
                let committed = snapshot(f).await;
                let mut req = valid_request();
                let mut existing = HeaderValue::from_str(&format!(
                    "{COOKIE}={token}; cyanrex_session=synthetic-old-teaching-session"
                ))
                .unwrap_or_else(|_| panic!("synthetic existing Cookie construction failed"));
                existing.set_sensitive(true);
                req.headers_mut().insert(header::COOKIE, existing);
                let preserved = capture(app.clone(), req).await;
                valid &= denied(
                    &preserved,
                    StatusCode::CONFLICT,
                    "session_present",
                    "not_attempted",
                ) && committed == snapshot(f).await;
            }
            let issued = token.is_some();
            drop(token);
            let count = session_count(f).await;
            let consumed = watermark(f).await >= 0;
            let acl = acl && prepared.minimal_acl().await;
            drop(response);
            drop(app);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                issued, exact, valid, acl, count, consumed, no_backend, cleaned,
            )
        })
    })
    .await;
    assert!(issued && exact && valid && acl && consumed && no_backend && cleaned);
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_wrong_missing_and_replayed_credentials_never_set_cookies() {
    let (refused, unchanged, issued, replay, retained, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let app = app(&prepared).await;
                let before = snapshot(f).await;
                let code = otp();
                let mut refused = true;
                for req in [
                    request(username().as_str(), "synthetic-wrong-password", &code),
                    request("synthetic-missing-http-user", password(), &code),
                    request(username().as_str(), password(), "not-six-digits"),
                ] {
                    let response = capture(app.clone(), req).await;
                    refused &= denied(
                        &response,
                        StatusCode::UNAUTHORIZED,
                        "invalid_credentials",
                        "unconfirmed",
                    );
                }
                let unchanged = before == snapshot(f).await
                    && session_count(f).await == 0
                    && watermark(f).await == -1;
                let response =
                    capture(app.clone(), request(username().as_str(), password(), &code)).await;
                let issued = issued_token(&response).is_some();
                drop(response);
                let committed = snapshot(f).await;
                let response =
                    capture(app.clone(), request(username().as_str(), password(), &code)).await;
                let replay = denied(
                    &response,
                    StatusCode::UNAUTHORIZED,
                    "invalid_credentials",
                    "unconfirmed",
                );
                let retained = committed == snapshot(f).await
                    && session_count(f).await == 1
                    && watermark(f).await >= 0;
                drop(response);
                drop(app);
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    refused, unchanged, issued, replay, retained, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(refused && unchanged && issued && replay && retained && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_http_full_stock_preserves_otp_without_cleanup_or_cookie() {
    let (refused, unchanged, explicit_retry, count, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            seed_session(f).await;
            seed_session(f).await;
            f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 hour'", f.source)).await;
            let app = app(&prepared).await;
            let before = snapshot(f).await;
            let code = otp();
            let response = capture(app.clone(), request(username().as_str(), password(), &code)).await;
            let refused = denied(&response, StatusCode::TOO_MANY_REQUESTS, "rate_limited", "unconfirmed");
            let unchanged = before == snapshot(f).await
                && session_count(f).await == 2 && watermark(f).await == -1;
            // A fixture owner explicitly removes one known synthetic row, never the HTTP issuer.
            f.sql(&format!("DELETE FROM {}.sessions WHERE token=(SELECT token FROM {}.sessions ORDER BY token LIMIT 1)", f.source, f.source)).await;
            let response = capture(app.clone(), request(username().as_str(), password(), &code)).await;
            let explicit_retry = issued_token(&response).is_some() && watermark(f).await >= 0;
            let count = session_count(f).await;
            drop(response);
            drop(app);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (refused, unchanged, explicit_retry, count, no_backend, cleaned)
        })
    }).await;
    assert!(refused && unchanged && explicit_retry && no_backend && cleaned);
    assert_eq!(count, 2);
}
