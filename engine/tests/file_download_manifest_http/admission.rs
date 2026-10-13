use crate::{fixture::*, offline::*, support::*, *};
use axum::body::HttpBody;

#[tokio::test]
async fn manifest_http_default_four_slots_clones_and_independent_states_have_actual_body_barriers()
{
    let f = Offline::new().await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(Held::start(f.app.clone(), TOKEN, &uri()).await);
    }
    saturated(&f.app, &uri()).await;
    for request in held {
        request.cancel().await;
    }
    available(&f.app).await;
    let state = f.state(1);
    let first = build_file_download_manifest_router(state.clone());
    let second = build_file_download_manifest_router(state);
    let independent = build_file_download_manifest_router(f.state(1));
    let held = Held::start(first.clone(), TOKEN, &uri()).await;
    saturated(&first, &uri()).await;
    saturated(&second, &uri()).await;
    available(&independent).await;
    held.cancel().await;
    available(&second).await;
}

#[tokio::test]
async fn manifest_http_full_admission_preserves_path_query_and_credentials_before_body_polling() {
    let f = Offline::new().await;
    let route = build_file_download_manifest_router(f.state(1));
    let held = Held::start(route.clone(), TOKEN, &uri()).await;
    for mode in [
        "path",
        "method",
        "query",
        "range",
        "marker",
        "origin",
        "cookie",
        "authorization",
        "valid",
    ] {
        let mut request = body_request(TOKEN, &uri(), unread());
        let (status, code) = match mode {
            "path" => {
                *request.uri_mut() = "/platform/v1/unknown".parse().unwrap();
                (StatusCode::NOT_FOUND, "not_found")
            }
            "method" => {
                *request.method_mut() = "POST".parse().unwrap();
                (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
            }
            "query" => {
                *request.uri_mut() = format!("{}&chunk_index=0", uri()).parse().unwrap();
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            "range" => {
                request
                    .headers_mut()
                    .insert("range", "bytes=0-1".parse().unwrap());
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            "marker" => {
                request.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                request
                    .headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "cookie" => {
                request.headers_mut().remove("cookie");
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "authorization" => {
                request
                    .headers_mut()
                    .insert("authorization", "Bearer private".parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            _ => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        };
        error(&route, request, status, code, "not_attempted").await;
    }
    held.cancel().await;
    available(&route).await;
}

#[tokio::test]
async fn manifest_http_admitted_json_error_releases_at_last_frame_or_drop_not_retained_data_clone()
{
    let f = Offline::new().await;
    let route = build_file_download_manifest_router(f.state(1));
    let response = route.clone().oneshot(request(TOKEN, &uri())).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    saturated(&route, &uri()).await;
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    let clone = frame.clone();
    assert!(body.is_end_stream());
    available(&route).await;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&clone).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}})
    );
    let response = route.clone().oneshot(request(TOKEN, &uri())).await.unwrap();
    saturated(&route, &uri()).await;
    drop(response);
    available(&route).await;
    drop((body, frame, clone));
}
