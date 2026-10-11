use super::*;

#[tokio::test]
async fn index_http_actual_empty_body_media_encoding_and_frames_are_checked_before_sql() {
    let f = Offline::new().await;
    for declared_empty in [false, true] {
        let mut req = request("GET", COLLECTION, "x");
        if declared_empty {
            req.headers_mut()
                .insert("content-length", "0".parse().unwrap());
        }
        rejected(&f, req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    for media in [
        "text/plain",
        "application/json; charset=latin1",
        "application/json, application/json",
    ] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut()
            .insert("content-type", media.parse().unwrap());
        rejected(
            &f,
            req,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        )
        .await;
    }
    let mut duplicate = request("GET", COLLECTION, unread_body());
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
    for encoding in ["gzip", "identity", ""] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut()
            .insert("content-encoding", encoding.parse().unwrap());
        rejected(&f, req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    for media in [
        None,
        Some("application/json"),
        Some("application/json; charset=utf-8"),
    ] {
        let mut req = request("GET", COLLECTION, Body::empty());
        req.headers_mut().remove("content-type");
        if let Some(media) = media {
            req.headers_mut()
                .insert("content-type", media.parse().unwrap());
        }
        error(
            f.app.clone(),
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
    let trailers = Body::new(http_body_util::StreamBody::new(futures_util::stream::once(
        async { Ok::<_, std::io::Error>(http_body::Frame::trailers(axum::http::HeaderMap::new())) },
    )));
    rejected(
        &f,
        request("GET", COLLECTION, trailers),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    let broken = Body::from_stream(futures_util::stream::once(async {
        Err::<axum::body::Bytes, _>(std::io::Error::other("synthetic private failure"))
    }));
    rejected(
        &f,
        request("GET", COLLECTION, broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}

#[tokio::test]
async fn index_http_two_second_body_deadline_bounds_pending_and_empty_frame_streams() {
    let f = Offline::new().await;
    for immediate_empty in [false, true] {
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            if immediate_empty {
                std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(axum::body::Bytes::new())))
            } else {
                std::task::Poll::Pending
            }
        }));
        tokio::time::timeout(
            Duration::from_secs(3),
            rejected(
                &f,
                request("GET", COLLECTION, body),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
            ),
        )
        .await
        .expect("whole-body deadline must yield even for empty immediately-ready frames");
    }
    available(f.app).await;
}
