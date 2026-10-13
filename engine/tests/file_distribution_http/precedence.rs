use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn distribution_http_cheap_path_method_selector_and_credentials_fail_before_both_full_gates()
{
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let state = f.bounded(1, 1);
            let route = build_file_distribution_router(state.clone());
            let clone = build_file_distribution_router(state);
            let manifest = Held::start(f, route.clone(), None).await;
            let chunk = Held::start(f, route.clone(), Some(0)).await;
            error(
                &clone,
                body_request("/auth/login", unread()),
                StatusCode::NOT_FOUND,
                "not_found",
                "not_attempted",
            )
            .await;
            for index in [None, Some(0)] {
                let path = f.path(index);
                let mut method = body_request(&path, unread());
                *method.method_mut() = "POST".parse().unwrap();
                error(
                    &clone,
                    method,
                    StatusCode::METHOD_NOT_ALLOWED,
                    "method_not_allowed",
                    "not_attempted",
                )
                .await;
                let missing = path.split_once('?').unwrap().0;
                let mut query = body_request(missing, unread());
                query.headers_mut().remove(MARKER);
                error(
                    &clone,
                    query,
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let mut conditional = body_request(&path, unread());
                conditional
                    .headers_mut()
                    .insert("if-unknown", "selector".parse().unwrap());
                conditional.headers_mut().remove(MARKER);
                error(
                    &clone,
                    conditional,
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let mut marker = body_request(&path, unread());
                marker.headers_mut().remove(MARKER);
                error(
                    &clone,
                    marker,
                    StatusCode::FORBIDDEN,
                    "request_forbidden",
                    "not_attempted",
                )
                .await;
                let mut cookie = body_request(&path, unread());
                cookie.headers_mut().remove("cookie");
                error(
                    &clone,
                    cookie,
                    StatusCode::UNAUTHORIZED,
                    "invalid_session",
                    "not_attempted",
                )
                .await;
                saturated(f, &clone, index).await;
            }
            manifest.cancel().await;
            chunk.cancel().await;
            for index in [None, Some(0)] {
                available(f, &clone, index).await;
            }
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
