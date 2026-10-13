use crate::{fixture::*, support::*, *};
use axum::body::Bytes;
use http_body::Frame;
use http_body_util::StreamBody;
use std::io;

#[tokio::test]
async fn distribution_http_collects_actual_empty_bodies_and_rejects_data_errors_trailers_and_fixed_deadline_evasion(
) {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            for index in [None, Some(0)] {
                let path = f.path(index);
                let mut lying = body_request(&path, Body::from("not-empty"));
                lying
                    .headers_mut()
                    .insert("content-length", "0".parse().unwrap());
                error(
                    &f.app,
                    lying,
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let failed = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
                    io::Error::other("synthetic actual request frame failure"),
                )]));
                error(
                    &f.app,
                    body_request(&path, failed),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let trailers = Body::new(StreamBody::new(futures_util::stream::iter([Ok::<
                    _,
                    io::Error,
                >(
                    Frame::<Bytes>::trailers(HeaderMap::new()),
                )])));
                error(
                    &f.app,
                    body_request(&path, trailers),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let mut encoded = body_request(&path, unread());
                encoded
                    .headers_mut()
                    .insert("content-encoding", "identity".parse().unwrap());
                error(
                    &f.app,
                    encoded,
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
                let mut media = body_request(&path, unread());
                media
                    .headers_mut()
                    .insert("content-type", "application/octet-stream".parse().unwrap());
                error(
                    &f.app,
                    media,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "unsupported_media_type",
                    "not_attempted",
                )
                .await;
                let empty = Body::from_stream(futures_util::stream::iter([
                    Ok::<_, io::Error>(Bytes::new()),
                    Ok(Bytes::new()),
                ]));
                let mut real_empty = body_request(&path, empty);
                real_empty
                    .headers_mut()
                    .insert("content-length", "1024".parse().unwrap());
                real_empty.headers_mut().insert(
                    "content-type",
                    "application/json; charset=utf-8".parse().unwrap(),
                );
                error(
                    &f.app,
                    real_empty,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                )
                .await;
            }
            // Two distinct actual frame sources exercise pending and always-ready-empty collection.
            let pending =
                Body::from_stream(futures_util::stream::pending::<Result<Bytes, io::Error>>());
            error(
                &f.app,
                body_request(&f.path(None), pending),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                "not_attempted",
            )
            .await;
            let endless = Body::from_stream(futures_util::stream::poll_fn(|_| {
                std::task::Poll::Ready(Some(Ok::<_, io::Error>(Bytes::new())))
            }));
            error(
                &f.app,
                body_request(&f.path(Some(0)), endless),
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
                "not_attempted",
            )
            .await;
            for index in [None, Some(0)] {
                available(f, &f.app, index).await;
            }
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
