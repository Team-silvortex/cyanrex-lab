use crate::{fixture::*, offline::*, support::*, *};
use std::time::Duration;
#[tokio::test]
async fn chunk_http_actual_empty_get_body_media_encoding_trailers_and_errors_are_checked() {
    let f = Offline::new().await;
    for declared in [false, true] {
        let mut r = request_body(TOKEN, &uri(), Body::from("x"));
        if declared {
            r.headers_mut()
                .insert("content-length", "0".parse().unwrap());
        }
        rejected(&f, r, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    for media in [
        "text/plain",
        "application/octet-stream",
        "application/json; charset=latin1",
        "application/json, application/json",
    ] {
        let mut r = request_body(TOKEN, &uri(), unread());
        r.headers_mut()
            .insert("content-type", media.parse().unwrap());
        rejected(
            &f,
            r,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        )
        .await;
    }
    let mut duplicate = request_body(TOKEN, &uri(), unread());
    duplicate
        .headers_mut()
        .append("content-type", "application/json".parse().unwrap());
    duplicate
        .headers_mut()
        .append("content-type", "application/json".parse().unwrap());
    rejected(
        &f,
        duplicate,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
    for encoding in ["identity", "gzip", ""] {
        let mut r = request_body(TOKEN, &uri(), unread());
        r.headers_mut()
            .insert("content-encoding", encoding.parse().unwrap());
        rejected(&f, r, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    let trailers = Body::new(http_body_util::StreamBody::new(futures_util::stream::once(
        async { Ok::<_, std::io::Error>(http_body::Frame::trailers(axum::http::HeaderMap::new())) },
    )));
    rejected(
        &f,
        request_body(TOKEN, &uri(), trailers),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    let broken = Body::from_stream(futures_util::stream::once(async {
        Err::<axum::body::Bytes, _>(std::io::Error::other("private request fault"))
    }));
    rejected(
        &f,
        request_body(TOKEN, &uri(), broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    for media in [
        None,
        Some("application/json"),
        Some("application/json; charset=utf-8"),
    ] {
        let mut r = request(TOKEN, &uri());
        if let Some(media) = media {
            r.headers_mut()
                .insert("content-type", media.parse().unwrap());
        }
        error(
            f.app.clone(),
            r,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}
#[tokio::test]
async fn chunk_http_two_second_request_deadline_bounds_pending_and_ready_empty_frames() {
    let f = Offline::new().await;
    for ready in [false, true] {
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            if ready {
                std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(axum::body::Bytes::new())))
            } else {
                std::task::Poll::Pending
            }
        }));
        tokio::time::timeout(
            Duration::from_secs(3),
            rejected(
                &f,
                request_body(TOKEN, &uri(), body),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
            ),
        )
        .await
        .expect("body collection must yield to its absolute deadline");
    }
    available(f.app).await;
}
