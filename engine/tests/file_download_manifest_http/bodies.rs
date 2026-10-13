use crate::{fixture::*, offline::*, support::*, *};
use std::time::Duration;

#[tokio::test]
async fn manifest_http_checks_actual_empty_body_media_encoding_error_and_trailer_before_source() {
    let f = Offline::new().await;
    for declared_empty in [false, true] {
        let mut request = body_request(TOKEN, &uri(), Body::from("private-body"));
        if declared_empty {
            request
                .headers_mut()
                .insert("content-length", "0".parse().unwrap());
        }
        rejected(&f, request, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    for media in [
        "text/plain",
        "application/octet-stream",
        "application/json; charset=latin1",
        "application/json, application/json",
    ] {
        let mut request = body_request(TOKEN, &uri(), unread());
        request
            .headers_mut()
            .insert("content-type", media.parse().unwrap());
        rejected(
            &f,
            request,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        )
        .await;
    }
    let mut duplicate = body_request(TOKEN, &uri(), unread());
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
        let mut request = body_request(TOKEN, &uri(), unread());
        request
            .headers_mut()
            .insert("content-encoding", encoding.parse().unwrap());
        rejected(&f, request, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    let trailers = Body::new(http_body_util::StreamBody::new(futures_util::stream::once(
        async { Ok::<_, std::io::Error>(http_body::Frame::trailers(axum::http::HeaderMap::new())) },
    )));
    rejected(
        &f,
        body_request(TOKEN, &uri(), trailers),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    let broken = Body::from_stream(futures_util::stream::once(async {
        Err::<axum::body::Bytes, _>(std::io::Error::other("private-request-fault"))
    }));
    rejected(
        &f,
        body_request(TOKEN, &uri(), broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    for media in [
        None,
        Some("application/json"),
        Some("application/json; charset=utf-8"),
    ] {
        let mut request = request(TOKEN, &uri());
        if let Some(media) = media {
            request
                .headers_mut()
                .insert("content-type", media.parse().unwrap());
        }
        error(
            &f.app,
            request,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}

#[tokio::test]
async fn manifest_http_two_second_body_deadline_bounds_pending_and_immediately_ready_empty_frames()
{
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
                body_request(TOKEN, &uri(), body),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
            ),
        )
        .await
        .expect("actual body polling must yield to its fixed deadline");
    }
    available(&f.app).await;
}
