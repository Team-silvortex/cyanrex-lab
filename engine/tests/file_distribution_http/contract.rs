use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn distribution_http_only_exact_two_get_paths_and_private_fallback_are_exposed_without_body_polling(
) {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            for path in [
                "/platform/v1/file-manifests/",
                "/platform/v1/file-manifests/extra",
                "/platform/v1/file-chunks/",
                "/platform/v1/file-chunks/extra",
                "/platform/v1/files",
                "/platform/v1/File-manifests",
                "/platform/v1/%66ile-manifests",
                "/platform/v1/%66ile-chunks",
                "/platform/v1/session",
                "/auth/login",
                "/ebpf",
                "/",
            ] {
                error(
                    &f.app,
                    body_request(path, unread()),
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "not_attempted",
                )
                .await;
            }
            for index in [None, Some(0)] {
                for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS", "TRACE", "HEAD"] {
                    let mut request = body_request(&f.path(index), unread());
                    *request.method_mut() = method.parse().unwrap();
                    let response = observe(&f.app, request).await;
                    assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
                    assert_eq!(response.headers["allow"], "GET");
                    if method == "HEAD" {
                        headers(&response.headers);
                        assert_eq!(response.headers["content-length"], "0");
                        assert!(response.body.is_empty());
                    } else {
                        error_observed(
                            response,
                            StatusCode::METHOD_NOT_ALLOWED,
                            "method_not_allowed",
                            "not_attempted",
                        );
                    }
                }
                available(f, &f.app, index).await;
            }
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}

#[tokio::test]
async fn distribution_http_keeps_six_manifest_and_seven_chunk_selectors_strictly_separate() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            for index in [None, Some(0)] {
                let path = f.path(index);
                let (endpoint, query) = path.split_once('?').unwrap();
                let fields = query.split('&').collect::<Vec<_>>();
                assert_eq!(fields.len(), if index.is_none() { 6 } else { 7 });
                let missing = format!("{endpoint}?{}", fields[1..].join("&"));
                let duplicate = format!("{path}&{}", fields[0]);
                let malformed = [
                    missing,
                    duplicate,
                    format!("{path}&owner_id=1"),
                    format!("{path}&scope=1"),
                    format!("{path}&path=chosen.bin"),
                    format!("{path}&token={TOKEN}"),
                    path.replace("byte_length=17", "byte_length=017"),
                    path.replace("byte_length=17", "byte_length=+17"),
                    path.replace("byte_length=17", "byte_length=%31%37"),
                    path.replace("byte_length=17", "byte_length=68719476737"),
                    path.replace(
                        "grant_id=00000000-0000-0000-0000-000000000065",
                        "grant_id=00000000-0000-0000-0000-000000000000",
                    ),
                    format!("{path}&{}", "x".repeat(513)),
                ];
                for malformed in malformed {
                    error(
                        &f.app,
                        body_request(&malformed, unread()),
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        "not_attempted",
                    )
                    .await;
                }
                let reversed = format!(
                    "{endpoint}?{}",
                    fields.into_iter().rev().collect::<Vec<_>>().join("&")
                );
                let mut native = request(&reversed);
                native.headers_mut().remove("origin");
                error(
                    &f.app,
                    native,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                )
                .await;
            }
            let manifest_with_index = format!("{}&chunk_index=0", f.path(None));
            error(
                &f.app,
                body_request(&manifest_with_index, unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
            let chunk_without_index = f.path(Some(0)).replace("&chunk_index=0", "");
            error(
                &f.app,
                body_request(&chunk_without_index, unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
            for index in ["00", "-1", "16384", "4294967296"] {
                let path = f
                    .path(Some(0))
                    .replace("chunk_index=0", &format!("chunk_index={index}"));
                error(
                    &f.app,
                    body_request(&path, unread()),
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "not_attempted",
                )
                .await;
            }
            available(f, &f.app, Some(16383)).await;
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}

#[tokio::test]
async fn distribution_http_preserves_exact_shared_cookie_origin_marker_and_reserved_header_policy()
{
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            for index in [None, Some(0)] {
                let path = f.path(index);
                for mode in 0..4 {
                    let mut request = body_request(&path, unread());
                    match mode {
                        0 => {
                            request.headers_mut().remove(MARKER);
                        }
                        1 => {
                            request.headers_mut().insert(MARKER, "01".parse().unwrap());
                        }
                        2 => {
                            request.headers_mut().append(MARKER, "1".parse().unwrap());
                        }
                        _ => {
                            request
                                .headers_mut()
                                .insert("origin", "https://foreign.example".parse().unwrap());
                        }
                    }
                    error(
                        &f.app,
                        request,
                        StatusCode::FORBIDDEN,
                        "request_forbidden",
                        "not_attempted",
                    )
                    .await;
                }
                for mode in 0..4 {
                    let mut request = body_request(&path, unread());
                    match mode {
                        0 => {
                            request.headers_mut().remove("cookie");
                        }
                        1 => {
                            request.headers_mut().insert(
                                "cookie",
                                format!("{COOKIE}={TOKEN}; {COOKIE}={TOKEN}")
                                    .parse()
                                    .unwrap(),
                            );
                        }
                        2 => {
                            request.headers_mut().insert(
                                "cookie",
                                format!("{COOKIE}={}", TOKEN.to_uppercase())
                                    .parse()
                                    .unwrap(),
                            );
                        }
                        _ => {
                            request
                                .headers_mut()
                                .insert("authorization", "Bearer forbidden".parse().unwrap());
                        }
                    }
                    error(
                        &f.app,
                        request,
                        StatusCode::UNAUTHORIZED,
                        "invalid_session",
                        "not_attempted",
                    )
                    .await;
                }
                for name in [
                    "range",
                    "if-range",
                    "if-match",
                    "if-none-match",
                    "if-modified-since",
                    "if-unmodified-since",
                    "if-unknown",
                ] {
                    let mut request = body_request(&path, unread());
                    request
                        .headers_mut()
                        .insert(name, "forbidden-selector".parse().unwrap());
                    error(
                        &f.app,
                        request,
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        "not_attempted",
                    )
                    .await;
                }
                let mut native = request(&path);
                native.headers_mut().remove("origin");
                error(
                    &f.app,
                    native,
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                )
                .await;
            }
            assert!(bytes::tree(&f.root).is_empty());
        })
    })
    .await;
}
