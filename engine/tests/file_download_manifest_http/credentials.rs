use crate::{fixture::*, offline::*, support::*, *};

#[tokio::test]
async fn manifest_http_range_and_all_conditional_selectors_are_rejected_before_body_or_source() {
    let f = Offline::new().await;
    for header in [
        "range",
        "if-range",
        "if-match",
        "if-none-match",
        "if-modified-since",
        "if-unmodified-since",
        "if-private",
    ] {
        let mut request = body_request(TOKEN, &uri(), unread());
        request.headers_mut().insert(
            axum::http::HeaderName::from_bytes(header.as_bytes()).unwrap(),
            "inert".parse().unwrap(),
        );
        rejected(&f, request, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
}

#[tokio::test]
async fn manifest_http_marker_origin_and_private_cookie_admission_are_canonical_and_redacted() {
    let f = Offline::new().await;
    for mode in [
        "missing-marker",
        "duplicate-marker",
        "bad-marker",
        "wrong-origin",
        "duplicate-origin",
        "authorization",
    ] {
        let mut request = body_request(TOKEN, &uri(), unread());
        let (status, code) = match mode {
            "authorization" => {
                request
                    .headers_mut()
                    .insert("authorization", "Bearer private-value".parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "missing-marker" => {
                request.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "duplicate-marker" => {
                request.headers_mut().append(MARKER, "1".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "bad-marker" => {
                request.headers_mut().insert(MARKER, "01".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "duplicate-origin" => {
                request
                    .headers_mut()
                    .append("origin", ORIGIN.parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            _ => {
                request
                    .headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
        };
        rejected(&f, request, status, code).await;
    }
    for cookie in [
        "",
        "cyanrex_platform_session",
        "cyanrex_platform_session=invalid",
        "cyanrex_platform_session=00000000-0000-0000-0000-000000000000",
    ] {
        let mut request = body_request(TOKEN, &uri(), unread());
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        rejected(&f, request, StatusCode::UNAUTHORIZED, "invalid_session").await;
    }
    let mut duplicate = body_request(TOKEN, &uri(), unread());
    duplicate
        .headers_mut()
        .append("cookie", format!("{COOKIE}={TOKEN}").parse().unwrap());
    rejected(&f, duplicate, StatusCode::UNAUTHORIZED, "invalid_session").await;
    let mut missing = body_request(TOKEN, &uri(), unread());
    missing.headers_mut().remove("cookie");
    rejected(&f, missing, StatusCode::UNAUTHORIZED, "invalid_session").await;
    let mut oversized = body_request(TOKEN, &uri(), unread());
    oversized.headers_mut().append(
        "cookie",
        format!("other={}", "x".repeat(8192)).parse().unwrap(),
    );
    rejected(
        &f,
        oversized,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
    )
    .await;
    let mut unrelated = request(TOKEN, &uri());
    unrelated
        .headers_mut()
        .append("cookie", "unrelated=inert".parse().unwrap());
    error(
        &f.app,
        unrelated,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
