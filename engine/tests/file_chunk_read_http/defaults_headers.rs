use crate::{fixture::*, offline::*, support::*, *};
#[tokio::test]
async fn chunk_http_dedicated_credentials_and_optional_exact_get_origin_are_required() {
    let f = Offline::new().await;
    let uri = uri();
    for mode in [
        "marker",
        "duplicatemarker",
        "wrongmarker",
        "origin",
        "duplicateorigin",
        "authorization",
        "cookie",
        "duplicatecookie",
        "badcookie",
        "legacycookie",
        "nilcookie",
        "uppercookie",
    ] {
        let mut r = request_body(TOKEN, &uri, unread());
        let (status, code) = match mode {
            "marker" => {
                r.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "duplicatemarker" => {
                r.headers_mut().append(MARKER, "1".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "wrongmarker" => {
                r.headers_mut().insert(MARKER, "true".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                r.headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "duplicateorigin" => {
                r.headers_mut().append("origin", ORIGIN.parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "authorization" => {
                r.headers_mut()
                    .insert("authorization", "Bearer ignored".parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "cookie" => {
                r.headers_mut().remove("cookie");
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "duplicatecookie" => {
                r.headers_mut()
                    .append("cookie", format!("{COOKIE}={TOKEN}").parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            mode => {
                let value = match mode {
                    "legacycookie" => format!("cyanrex_session={TOKEN}"),
                    "nilcookie" => format!("{COOKIE}={}", Uuid::nil()),
                    "uppercookie" => format!("{COOKIE}={}", TOKEN.to_uppercase()),
                    _ => format!("{COOKIE}=not-a-session"),
                };
                r.headers_mut().insert("cookie", value.parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
        };
        rejected(&f, r, status, code).await;
    }
    let mut oversized = request_body(TOKEN, &uri, unread());
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
    let mut native = request(TOKEN, &uri);
    native.headers_mut().remove("origin");
    error(
        f.app.clone(),
        native,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    available(f.app).await;
}
#[tokio::test]
async fn chunk_http_rejects_all_range_and_conditional_headers_without_polling_or_dispatch() {
    let f = Offline::new().await;
    for header in [
        "range",
        "if-range",
        "if-none-match",
        "if-match",
        "if-modified-since",
        "if-unmodified-since",
    ] {
        for value in ["", "bytes=0-16", "ignored"] {
            let mut r = request_body(TOKEN, &uri(), unread());
            r.headers_mut().insert(header, value.parse().unwrap());
            rejected(&f, r, StatusCode::BAD_REQUEST, "invalid_request").await;
        }
    }
    available(f.app).await;
}
