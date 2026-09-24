use super::support::*;
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn event_settings_http_is_private_and_preserves_validation_and_clamping() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let alice = f.cookie("network-alice").await;
    for (method, payload, status) in [
        ("GET", "null", StatusCode::OK),
        (
            "POST",
            "{\"max_records\":0,\"overflow_policy\":\"drop_new\",\"username\":\"network-bob\"}",
            StatusCode::OK,
        ),
        (
            "POST",
            "{\"max_records\":50001,\"overflow_policy\":\"drop_oldest\"}",
            StatusCode::OK,
        ),
        (
            "POST",
            "{\"max_records\":-1,\"overflow_policy\":\"drop_new\"}",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            "{\"max_records\":50,\"overflow_policy\":\"typo\"}",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("POST", "{}", StatusCode::UNPROCESSABLE_ENTITY),
        ("POST", "{", StatusCode::BAD_REQUEST),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/settings/events")
                    .header(header::COOKIE, &alice)
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store")
        );
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        if !status.is_success() {
            assert_eq!(
                body,
                json!({"ok":false,"message":"invalid event settings request","settings":null})
            );
        } else if method == "POST" {
            assert_eq!(
                body["settings"]["max_records"],
                if payload.contains("50001") { 50000 } else { 50 }
            );
        }
    }
    for (content_type, payload, status) in [
        (
            "text/plain",
            "{}".to_owned(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "application/json",
            " ".repeat(2 * 1024 * 1024 + 1),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/settings/events")
                    .header(header::COOKIE, &alice)
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(
            body,
            json!({"ok":false,"message":"invalid event settings request","settings":null})
        );
    }
    assert_eq!(
        f.get("/settings/events", &alice).await["max_records"],
        50000
    );
    for (method, cookie, origin, status) in [
        ("GET", "", ORIGIN, StatusCode::UNAUTHORIZED),
        ("POST", "", ORIGIN, StatusCode::UNAUTHORIZED),
        (
            "POST",
            alice.as_str(),
            "https://untrusted.invalid",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/settings/events")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, origin)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        "{\"max_records\":50,\"overflow_policy\":\"drop_new\"}",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    assert_eq!(
        f.get("/settings/events", &alice).await["max_records"],
        50000
    );
}
