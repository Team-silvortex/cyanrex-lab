use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
    response::Response,
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::Poll,
    time::{Duration, Instant},
};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:43127";
const HTML: &str = "/platform/";
const PATHS: [(&str, &str); 7] = [
    (HTML, "text/html; charset=utf-8"),
    ("/platform/ui/app.js", "text/javascript; charset=utf-8"),
    ("/platform/ui/labels.js", "text/javascript; charset=utf-8"),
    ("/platform/ui/style.css", "text/css; charset=utf-8"),
    ("/platform/ui/tasks.js", "text/javascript; charset=utf-8"),
    (
        "/platform/ui/task-edit.js",
        "text/javascript; charset=utf-8",
    ),
    (
        "/platform/ui/task-contract.js",
        "text/javascript; charset=utf-8",
    ),
];

fn request(method: Method, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .body(body)
        .unwrap()
}

async fn capture(route: Router, request: Request<Body>) -> (StatusCode, HeaderMap, Bytes) {
    let response = tokio::time::timeout(Duration::from_secs(4), route.oneshot(request))
        .await
        .expect("owned static route exceeded the test deadline")
        .unwrap();
    capture_response(response).await
}

async fn capture_response(response: Response) -> (StatusCode, HeaderMap, Bytes) {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = tokio::time::timeout(Duration::from_secs(1), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    (status, headers, bytes)
}

fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("cheap static refusal must not poll the request body")
        },
    ))
}

fn private(headers: &HeaderMap) {
    for (name, expected) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cross-origin-opener-policy", "same-origin"),
        ("x-frame-options", "DENY"),
    ] {
        assert_eq!(headers[name], expected, "{name}");
    }
    for name in [
        "set-cookie",
        "location",
        "access-control-allow-origin",
        "access-control-allow-credentials",
        "www-authenticate",
        "retry-after",
    ] {
        assert!(
            !headers.contains_key(name),
            "static response must not emit {name}"
        );
    }
    let policy = headers["content-security-policy"].to_str().unwrap();
    let directives: Vec<_> = policy
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    for required in [
        "default-src 'none'",
        "script-src 'self'",
        "style-src 'self'",
        "connect-src 'self'",
        "base-uri 'none'",
        "form-action 'none'",
        "frame-ancestors 'none'",
        "worker-src 'none'",
        "object-src 'none'",
    ] {
        assert!(
            directives.contains(&required),
            "missing fixed CSP directive {required}"
        );
    }
    assert!(!policy.contains("unsafe-inline") && !policy.contains("unsafe-eval"));
    assert!(!policy.contains('*'));
}

async fn error(req: Request<Body>, expected: StatusCode, code: &str) -> HeaderMap {
    let (status, headers, bytes) = capture(super::routes(ORIGIN), req).await;
    assert_eq!(status, expected);
    private(&headers);
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({
            "schema_version":1,"error":{"code":code,"outcome":"not_attempted"}
        })
    );
    headers
}

#[tokio::test]
async fn session_ui_explicit_html_route_is_real_html_not_the_original_json_fallback() {
    let (status, headers, body) = capture(
        super::routes(ORIGIN),
        request(Method::GET, HTML, Body::empty()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
    assert!(std::str::from_utf8(&body).unwrap().contains("<html"));
}

#[tokio::test]
async fn session_ui_closed_assets_have_correct_mime_and_private_response_policy() {
    for (path, mime) in PATHS {
        let (status, headers, bytes) = capture(
            super::routes(ORIGIN),
            request(Method::GET, path, Body::empty()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(headers[header::CONTENT_TYPE], mime);
        private(&headers);
        assert!(!bytes.is_empty() && bytes.len() < 64 * 1024);
        assert!(std::str::from_utf8(&bytes).is_ok());
    }
}

#[tokio::test]
async fn session_ui_static_html_uses_only_external_closed_script_and_style_paths() {
    let (_, _, bytes) = capture(
        super::routes(ORIGIN),
        request(Method::GET, HTML, Body::empty()),
    )
    .await;
    let html = std::str::from_utf8(&bytes).unwrap();
    for path in ["/platform/ui/app.js", "/platform/ui/style.css"] {
        assert!(html.contains(path), "fixed HTML did not select {path}");
    }
    for forbidden in [
        "<script>",
        "onclick=",
        "onsubmit=",
        "onload=",
        "<iframe",
        "<base",
        "javascript:",
    ] {
        assert!(
            !html.to_ascii_lowercase().contains(forbidden),
            "HTML contains {forbidden}"
        );
    }
}

#[tokio::test]
async fn session_ui_present_fetch_destination_and_mode_must_match_the_fixed_asset_family() {
    for (path, _) in PATHS {
        for (name, value) in [
            ("sec-fetch-dest", "iframe"),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-mode", "websocket"),
            ("sec-fetch-mode", "same-origin"),
        ] {
            let mut req = request(Method::GET, path, unread());
            req.headers_mut().insert(name, value.parse().unwrap());
            error(req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        for name in ["sec-fetch-dest", "sec-fetch-mode"] {
            let mut req = request(Method::GET, path, unread());
            let value = if name == "sec-fetch-dest" {
                if path == HTML {
                    "document"
                } else if path.ends_with(".css") {
                    "style"
                } else {
                    "script"
                }
            } else if path == HTML {
                "navigate"
            } else {
                "no-cors"
            };
            req.headers_mut().append(name, value.parse().unwrap());
            req.headers_mut().append(name, value.parse().unwrap());
            error(req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        let mut valid = request(Method::GET, path, Body::empty());
        let dest = if path == HTML {
            "document"
        } else if path.ends_with(".css") {
            "style"
        } else {
            "script"
        };
        valid
            .headers_mut()
            .insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        valid
            .headers_mut()
            .insert("sec-fetch-dest", dest.parse().unwrap());
        valid.headers_mut().insert(
            "sec-fetch-mode",
            if path == HTML { "navigate" } else { "no-cors" }
                .parse()
                .unwrap(),
        );
        assert_eq!(
            capture(super::routes(ORIGIN), valid).await.0,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn session_ui_encoding_and_media_headers_are_rejected_before_body_polling() {
    for (path, _) in PATHS {
        for (name, value) in [
            (header::CONTENT_ENCODING, "identity"),
            (header::CONTENT_ENCODING, "gzip"),
            (header::CONTENT_TYPE, "application/json"),
            (header::CONTENT_TYPE, "text/plain"),
        ] {
            let mut req = request(Method::GET, path, unread());
            req.headers_mut().insert(name, value.parse().unwrap());
            error(req, StatusCode::BAD_REQUEST, "invalid_request").await;
        }
    }
}

#[tokio::test]
async fn session_ui_query_and_non_get_methods_fail_before_body_poll_with_no_redirect() {
    for (path, _) in PATHS {
        for query in [
            "?",
            "?x=1",
            "?next=https://untrusted.invalid",
            "?token=synthetic-private",
        ] {
            error(
                request(Method::GET, &format!("{path}{query}"), unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
        for method in [
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ] {
            let headers = error(
                request(method, path, unread()),
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
            )
            .await;
            assert_eq!(headers[header::ALLOW], "GET");
        }
    }
}

#[tokio::test]
async fn session_ui_head_is_empty_405_not_automatic_get_or_credential_probe() {
    for (path, _) in PATHS {
        let (status, headers, bytes) =
            capture(super::routes(ORIGIN), request(Method::HEAD, path, unread())).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        private(&headers);
        assert_eq!(headers[header::ALLOW], "GET");
        assert_eq!(headers[header::CONTENT_LENGTH], "0");
        assert!(bytes.is_empty());
    }
}

#[tokio::test]
async fn session_ui_optional_origin_must_be_single_and_exact_before_any_body_read() {
    for (path, _) in PATHS {
        for origin in [
            "null",
            "https://127.0.0.1:43127",
            "http://127.0.0.1:43128",
            "http://localhost:43127",
            "http://127.0.0.1:43127/",
            "https://untrusted.invalid",
        ] {
            let mut req = request(Method::GET, path, unread());
            req.headers_mut()
                .insert(header::ORIGIN, origin.parse().unwrap());
            error(req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        let mut duplicate = request(Method::GET, path, unread());
        duplicate
            .headers_mut()
            .append(header::ORIGIN, ORIGIN.parse().unwrap());
        duplicate
            .headers_mut()
            .append(header::ORIGIN, ORIGIN.parse().unwrap());
        error(duplicate, StatusCode::FORBIDDEN, "request_forbidden").await;
        let mut valid = request(Method::GET, path, Body::empty());
        valid
            .headers_mut()
            .insert(header::ORIGIN, ORIGIN.parse().unwrap());
        assert_eq!(
            capture(super::routes(ORIGIN), valid).await.0,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn session_ui_fetch_site_is_defense_in_depth_not_native_probe_authentication() {
    for (path, _) in PATHS {
        for site in [
            "cross-site",
            "same-site",
            "Same-Origin",
            "same-origin, none",
            "",
            "unknown",
        ] {
            let mut req = request(Method::GET, path, unread());
            req.headers_mut()
                .insert("sec-fetch-site", site.parse().unwrap());
            error(req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        let mut duplicate = request(Method::GET, path, unread());
        duplicate
            .headers_mut()
            .append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        duplicate
            .headers_mut()
            .append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        error(duplicate, StatusCode::FORBIDDEN, "request_forbidden").await;
        for site in [None, Some("same-origin")] {
            let mut req = request(Method::GET, path, Body::empty());
            if let Some(site) = site {
                req.headers_mut()
                    .insert("sec-fetch-site", site.parse().unwrap());
            }
            assert_eq!(capture(super::routes(ORIGIN), req).await.0, StatusCode::OK);
        }
    }
    let mut navigation = request(Method::GET, HTML, Body::empty());
    navigation
        .headers_mut()
        .insert("sec-fetch-site", HeaderValue::from_static("none"));
    navigation
        .headers_mut()
        .insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
    navigation
        .headers_mut()
        .insert("sec-fetch-dest", HeaderValue::from_static("document"));
    assert_eq!(
        capture(super::routes(ORIGIN), navigation).await.0,
        StatusCode::OK
    );
    for (path, _) in &PATHS[1..] {
        let mut req = request(Method::GET, path, unread());
        req.headers_mut()
            .insert("sec-fetch-site", HeaderValue::from_static("none"));
        error(req, StatusCode::FORBIDDEN, "request_forbidden").await;
    }
}

#[tokio::test]
async fn session_ui_authorization_is_refused_but_cookie_values_are_never_authentication_or_echo() {
    for (path, _) in PATHS {
        let mut auth = request(Method::GET, path, unread());
        auth.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer synthetic-not-a-session"),
        );
        error(auth, StatusCode::FORBIDDEN, "request_forbidden").await;
        let mut req = request(Method::GET, path, Body::empty());
        req.headers_mut().append(
            header::COOKIE,
            HeaderValue::from_static("cyanrex_platform_session=malformed; cyanrex_session=legacy"),
        );
        req.headers_mut()
            .append(header::COOKIE, HeaderValue::from_bytes(&[0xff]).unwrap());
        let (status, headers, body) = capture(super::routes(ORIGIN), req).await;
        assert_eq!(status, StatusCode::OK);
        private(&headers);
        assert!(!String::from_utf8_lossy(&body).contains("synthetic-not-a-session"));
        assert!(!String::from_utf8_lossy(&body).contains("cyanrex_platform_session=malformed"));
    }
}

#[tokio::test]
async fn session_ui_nonempty_actual_body_errors_and_trailers_are_rejected_even_with_zero_length_claims(
) {
    use http_body::Frame;
    use http_body_util::StreamBody;
    for (path, _) in PATHS {
        let mut req = request(Method::GET, path, Body::from("synthetic-unread-secret"));
        req.headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        error(req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    let broken = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
        std::io::Error::other("private fixture error"),
    )]));
    error(
        request(Method::GET, HTML, broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    let mut trailers = HeaderMap::new();
    trailers.insert("x-private-trailer", HeaderValue::from_static("not-echoed"));
    let body = Body::new(StreamBody::new(futures_util::stream::iter([Ok::<
        _,
        std::io::Error,
    >(
        Frame::<Bytes>::trailers(trailers),
    )])));
    error(
        request(Method::GET, HTML, body),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}

#[tokio::test]
async fn session_ui_finite_empty_frames_are_collected_to_real_end_without_requiring_json_media() {
    for count in [0, 1, 10] {
        let frames = (0..count)
            .map(|_| Ok::<_, std::io::Error>(Bytes::new()))
            .collect::<Vec<_>>();
        let req = request(
            Method::GET,
            HTML,
            Body::from_stream(futures_util::stream::iter(frames)),
        );
        let (status, headers, _) = capture(super::routes(ORIGIN), req).await;
        assert_eq!(status, StatusCode::OK);
        private(&headers);
    }
}

#[tokio::test]
async fn session_ui_pending_body_is_actually_polled_and_has_a_two_second_whole_body_deadline() {
    let polls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&polls);
    let body = Body::from_stream(futures_util::stream::poll_fn(
        move |_| -> Poll<Option<Result<Bytes, std::io::Error>>> {
            observed.fetch_add(1, Ordering::SeqCst);
            Poll::Pending
        },
    ));
    let started = Instant::now();
    error(
        request(Method::GET, HTML, body),
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
    )
    .await;
    assert!(polls.load(Ordering::SeqCst) > 0);
    assert!(started.elapsed() >= Duration::from_millis(1900));
}

#[tokio::test]
async fn session_ui_immediately_ready_empty_frames_yield_so_the_deadline_cannot_be_starved() {
    let started = Instant::now();
    let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
        // A finite wall-clock canary prevents a missing product yield from hanging this test.
        if started.elapsed() > Duration::from_secs(3) {
            Poll::Ready(Some(Err(std::io::Error::other(
                "bounded empty-frame fixture exhausted",
            ))))
        } else {
            Poll::Ready(Some(Ok(Bytes::new())))
        }
    }));
    error(
        request(Method::GET, HTML, body),
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
    )
    .await;
}

#[tokio::test]
async fn session_ui_unknown_paths_preserve_the_original_router_fallback_without_body_polling() {
    async fn fallback() -> (StatusCode, axum::Json<Value>) {
        (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"schema_version":1,
            "error":{"code":"not_found","outcome":"not_attempted"}})),
        )
    }
    let app = Router::new()
        .fallback(fallback)
        .merge(super::routes(ORIGIN));
    for path in [
        "/",
        "/platform",
        "/platform//",
        "/platform/index.html",
        "/platform/ui/",
        "/platform/ui/../app.js",
        "/platform/ui/%61pp.js",
        "/platform/ui/app.js/",
        "/platform/ui/secrets.env",
        "/platform/other",
        "/auth",
        "/ebpf",
    ] {
        let (status, headers, bytes) =
            capture(app.clone(), request(Method::GET, path, unread())).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({"schema_version":1,
            "error":{"code":"not_found","outcome":"not_attempted"}})
        );
    }
}
