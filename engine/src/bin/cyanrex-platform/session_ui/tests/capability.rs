use super::*;

const DISABLED: &str = "data-task-text-publication=\"disabled\"";
const ENABLED: &str = "data-task-text-publication=\"enabled\"";

#[tokio::test]
async fn session_ui_task_publication_is_default_off_and_changes_only_one_trusted_html_marker() {
    let (_, mut default_headers, default) = capture(
        super::super::routes(ORIGIN),
        request(Method::GET, HTML, Body::empty()),
    )
    .await;
    let default = std::str::from_utf8(&default).unwrap();
    assert_eq!(default.matches(DISABLED).count(), 1);
    assert!(!default.contains(ENABLED));
    default_headers.remove(header::CONTENT_LENGTH);
    for enabled in [false, true] {
        let (status, mut headers, bytes) = capture(
            super::super::with_task_text_publication(ORIGIN, enabled),
            request(Method::GET, HTML, Body::empty()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        private(&headers);
        assert_eq!(headers[header::CONTENT_LENGTH], bytes.len().to_string());
        headers.remove(header::CONTENT_LENGTH);
        assert_eq!(headers, default_headers);
        let html = std::str::from_utf8(&bytes).unwrap();
        if enabled {
            assert_eq!(html.matches(ENABLED).count(), 1);
            assert!(!html.contains(DISABLED));
            assert_eq!(html.replace(ENABLED, DISABLED), default);
        } else {
            assert_eq!(html, default);
        }
    }
}

#[tokio::test]
async fn session_ui_task_publication_preserves_every_static_asset_byte_mime_and_csp() {
    for (path, mime) in &PATHS[1..] {
        let original = capture(
            super::super::routes(ORIGIN),
            request(Method::GET, path, Body::empty()),
        )
        .await;
        assert_eq!(original.0, StatusCode::OK, "{path}");
        for enabled in [false, true] {
            let observed = capture(
                super::super::with_task_text_publication(ORIGIN, enabled),
                request(Method::GET, path, Body::empty()),
            )
            .await;
            assert_eq!(observed, original, "{path}");
            assert_eq!(observed.1[header::CONTENT_TYPE], *mime);
            private(&observed.1);
        }
    }
}

#[tokio::test]
async fn session_ui_task_publication_client_inputs_cannot_select_the_host_capability() {
    for enabled in [false, true] {
        let app = super::super::with_task_text_publication(ORIGIN, enabled);
        let mut req = request(Method::GET, HTML, Body::empty());
        req.headers_mut().insert(
            "x-task-text-publication",
            HeaderValue::from_static(if enabled { "disabled" } else { "enabled" }),
        );
        let (status, headers, bytes) = capture(app.clone(), req).await;
        assert_eq!(status, StatusCode::OK);
        private(&headers);
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains(if enabled { ENABLED } else { DISABLED }));
        let (status, _, _) = capture(
            app,
            request(
                Method::GET,
                "/platform/?task_text_publication=enabled",
                unread(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn session_ui_task_publication_does_not_change_merged_api_routes_or_api_headers() {
    async fn api() -> (
        StatusCode,
        [(header::HeaderName, &'static str); 1],
        &'static str,
    ) {
        (
            StatusCode::ACCEPTED,
            [(header::CONTENT_TYPE, "application/json")],
            "fixed-api",
        )
    }
    for enabled in [false, true] {
        let app = Router::new()
            .route("/platform/v1/session", axum::routing::get(api))
            .merge(super::super::with_task_text_publication(ORIGIN, enabled));
        let (status, headers, bytes) = capture(
            app.clone(),
            request(Method::GET, "/platform/v1/session", Body::empty()),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert!(!headers.contains_key("content-security-policy"));
        assert!(!headers.contains_key(header::SET_COOKIE));
        assert_eq!(bytes, "fixed-api");
        let (status, _, _) = capture(
            app,
            request(
                Method::POST,
                "/platform/v1/tasks/00000000-0000-4000-8000-000000000003/text-artifacts",
                unread(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
