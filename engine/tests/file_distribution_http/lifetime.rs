use crate::{fixture::*, support::*, *};
use axum::body::HttpBody;
use std::time::Duration;

#[tokio::test]
async fn distribution_http_request_cancellation_and_error_body_drop_or_last_frame_release_only_the_matching_http_budget(
) {
    checked(Fixture::new().await, |f| Box::pin(async move {
        let route = build_file_distribution_router(f.bounded(1, 1));
        for index in [None, Some(0)] {
            let other = if index.is_none() { Some(0) } else { None };
            let held = Held::start(f, route.clone(), index).await;
            saturated(f, &route, index).await;
            available(f, &route, other).await;
            held.cancel().await;
            available(f, &route, index).await;

            let abandoned = Held::start(f, route.clone(), index).await;
            drop(abandoned);
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    tokio::task::yield_now().await;
                    let response = observe(&route, request(&f.path(index))).await;
                    let value: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
                    if value["error"]["code"] == "capacity_exhausted" {
                        error_observed(response, StatusCode::SERVICE_UNAVAILABLE,
                            "capacity_exhausted", "not_attempted");
                    } else {
                        error_observed(response, StatusCode::SERVICE_UNAVAILABLE,
                            "service_unavailable", "unconfirmed");
                        break;
                    }
                }
            }).await.expect("dropped owned waiter eventually releases its actual HTTP permit");

            let response = raw_response(&route, request(&f.path(index))).await;
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            saturated(f, &route, index).await;
            available(f, &route, other).await;
            drop(response);
            available(f, &route, index).await;

            let held = Held::start(f, route.clone(), index).await;
            let response = held.finish().await;
            saturated(f, &route, index).await;
            let mut body = response.into_body();
            let data = tokio::time::timeout(Duration::from_secs(3), body.frame()).await
                .expect("actual first error JSON frame is bounded").unwrap().unwrap().into_data().unwrap();
            let copy = data.clone();
            let slice = data.slice(0..1);
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&data).unwrap(),
                serde_json::json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}}));
            // This JSON's terminal frame releases HTTP admission even while handed-off Bytes live.
            assert!(body.is_end_stream());
            available(f, &route, index).await;
            assert_eq!(data, copy);
            assert!(!slice.is_empty());
            drop((body, data, copy, slice));
            available(f, &route, index).await;
        }
        assert!(bytes::tree(&f.root).is_empty());
    })).await;
}
