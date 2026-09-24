use super::support::*;
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn event_mutation_success_and_invalid_queries_are_private_json() {
    let _guard = ENV_LOCK.lock().await;
    let f = Fixture::new();
    let cookie = f.cookie("network-alice").await;
    for (path, status) in [
        ("/events/mark-read", StatusCode::OK),
        ("/events/delete", StatusCode::OK),
        ("/events/delete?category=typo", StatusCode::BAD_REQUEST),
        ("/events/delete?sevrity=error", StatusCode::BAD_REQUEST),
        ("/events/delete?since_minutes=oops", StatusCode::BAD_REQUEST),
        (
            "/events/delete?category=kernel&category=platform",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::COOKIE, &cookie)
                    .header(header::ORIGIN, ORIGIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-store"),
            "{path}"
        );
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["ok"], status == StatusCode::OK);
    }
}
