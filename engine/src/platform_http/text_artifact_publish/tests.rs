use super::{
    test_support::*, Backend as PublishBackend, TaskContentHttpConfigError,
    TextArtifactPublishHttpState,
};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, HeaderValue, StatusCode},
};
use serde_json::json;

#[test]
fn text_publish_configuration_checks_origin_before_capacity() {
    for capacity in [1, 4, 16] {
        assert!(TextArtifactPublishHttpState::configured(
            PublishBackend::Test(Backend::new(Outcome::Success)),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TextArtifactPublishHttpState::configured(
                PublishBackend::Test(Backend::new(Outcome::Success)),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            TextArtifactPublishHttpState::configured(
                PublishBackend::Test(Backend::new(Outcome::Success)),
                "http://platform.example",
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn text_publish_returns_only_confirmed_revision_and_forwards_exact_owned_inputs() {
    let backend = Backend::new(Outcome::Success);
    let response = capture(app(backend.clone(), 4), request("POST", CREATE, body())).await;
    assert_eq!(response.status, StatusCode::CREATED);
    private(&response);
    assert_eq!(
        response.json,
        json!({"schema_version":1,"artifact":published(ID.parse().unwrap(), REVISION.parse().unwrap(), TITLE, TEXT)})
    );
    assert!(response.json.get("text").is_none());
    assert!(response.json.get("task").is_none());
    assert_eq!(backend.calls(), 1);
    let received = backend.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].token, TOKEN);
    assert_eq!(received[0].id, ID.parse().unwrap());
    assert_eq!(received[0].revision, REVISION.parse().unwrap());
    assert_eq!(received[0].title, TITLE);
    assert_eq!(received[0].text, TEXT);
}

#[tokio::test]
async fn text_publish_route_is_exact_post_only_without_read_or_other_capabilities() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for path in [
        "/",
        "/platform/v1/text-artifacts/",
        "/platform/v1/text%2dartifacts",
        "/platform/v1/text-artifacts/extra",
        "/platform/v1/tasks",
        "/platform/v1/task-index",
        "/platform/v1/session/login",
        "/platform/v1/file-chunks",
    ] {
        error(
            &capture(app.clone(), request("POST", path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    let read = format!("{CREATE}/{ID}/revisions/{REVISION}/{}", "a".repeat(64));
    error(
        &capture(app.clone(), request("GET", &read, untouched())).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
    for method in ["GET", "PUT", "PATCH", "DELETE", "OPTIONS"] {
        let response = capture(app.clone(), request(method, CREATE, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "POST");
    }
    let head = capture(app.clone(), request("HEAD", CREATE, untouched())).await;
    assert_eq!(head.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(head.headers[header::ALLOW], "POST");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    private(&head);
    for query in ["?", "?owner=other", "?token=private", "?revision=1"] {
        error(
            &capture(
                app.clone(),
                request("POST", &format!("{CREATE}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn text_publish_rejects_missing_duplicate_or_foreign_ingress_headers_before_body() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for name in ["origin", "x-cyanrex-platform-request"] {
        for duplicate in [false, true] {
            let mut req = request("POST", CREATE, untouched());
            if duplicate {
                req.headers_mut().append(
                    name,
                    HeaderValue::from_static(if name == "origin" { ORIGIN } else { "1" }),
                );
            } else {
                req.headers_mut().remove(name);
            }
            error(
                &capture(app.clone(), req).await,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            );
        }
    }
    for (name, value) in [
        ("origin", "https://foreign.example"),
        ("x-cyanrex-platform-request", "true"),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().insert(name, value.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn text_publish_requires_only_dedicated_cookie_and_never_bearer_or_legacy_fallback() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for cookie in [
        None,
        Some("cyanrex_session=synthetic".into()),
        Some("cyanrex_platform_session=not-a-token".into()),
        Some(format!(
            "cyanrex_platform_session={TOKEN}; cyanrex_platform_session={TOKEN}"
        )),
        Some(format!("cyanrex_platform_session={}", TOKEN.to_uppercase())),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().remove(header::COOKIE);
        if let Some(cookie) = cookie {
            req.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    for authorization in ["", "Bearer ignored"] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut()
            .insert(header::AUTHORIZATION, authorization.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    let mut req = request("POST", CREATE, untouched());
    req.headers_mut().append(
        header::COOKIE,
        format!("cyanrex_platform_session={TOKEN}").parse().unwrap(),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut req = request("POST", CREATE, untouched());
    req.headers_mut().append(
        header::COOKIE,
        format!("unrelated={}", "x".repeat(8193)).parse().unwrap(),
    );
    error(
        &capture(app, req).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn text_publish_reuses_strict_five_field_dto_without_client_authority_or_aliases() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let valid = wire();
    let raw = valid.to_string();
    let mut invalid = vec![
        "null".into(),
        "[]".into(),
        "{}".into(),
        "\"text\"".into(),
        format!("[1,\"{ID}\",\"{REVISION}\",\"Title\",\"text\"]"),
        format!("\u{feff}{raw}"),
        format!("{raw} {{}}"),
    ];
    for key in [
        "schema_version",
        "artifact_id",
        "revision_id",
        "title",
        "text",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(key);
        invalid.push(missing.to_string());
        let duplicate = raw.replacen(
            &format!("\"{key}\":"),
            &format!("\"{key}\":null,\"{key}\":"),
            1,
        );
        assert_ne!(
            duplicate, raw,
            "duplicate field fixture must actually change the wire"
        );
        invalid.push(duplicate);
        let mut null = valid.clone();
        null[key] = json!(null);
        invalid.push(null.to_string());
    }
    for version in [json!(0), json!(2), json!(1.0), json!("1"), json!(-1)] {
        let mut changed = valid.clone();
        changed["schema_version"] = version;
        invalid.push(changed.to_string());
    }
    for key in ["artifact_id", "revision_id"] {
        for value in [
            "not-an-id".into(),
            ID.to_uppercase(),
            ID.replace('-', ""),
            "00000000-0000-0000-0000-000000000000".into(),
        ] {
            let mut changed = valid.clone();
            changed[key] = json!(value);
            invalid.push(changed.to_string());
        }
    }
    for key in [
        "owner",
        "scope",
        "namespace",
        "workspace",
        "path",
        "sha256",
        "kind",
        "media_type",
        "filename",
        "language",
        "parent",
        "sequence",
        "task",
        "run",
        "expected_revision",
    ] {
        let mut changed = valid.clone();
        changed[key] = json!("not client authority");
        invalid.push(changed.to_string());
    }
    for key in ["title", "text"] {
        let mut changed = valid.clone();
        changed[key] = json!(17);
        invalid.push(changed.to_string());
    }
    for raw in invalid {
        error(
            &capture(app.clone(), request("POST", CREATE, Body::from(raw))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for raw in [
        vec![0xff, 0xfe],
        wire()
            .to_string()
            .replace("Private notes", "\\ud800")
            .into_bytes(),
    ] {
        error(
            &capture(app.clone(), request("POST", CREATE, Body::from(raw))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    assert_eq!(
        capture(app, request("POST", CREATE, body())).await.status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn text_publish_title_and_decoded_utf8_profile_boundaries_are_independent_of_wire_limit() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for title in [
        "".into(),
        " \t".into(),
        "x".repeat(257),
        "😀".repeat(65),
        "bad\nname".into(),
        "bad\u{85}".into(),
    ] {
        let mut changed = wire();
        changed["title"] = json!(title);
        error(
            &capture(
                app.clone(),
                request("POST", CREATE, Body::from(changed.to_string())),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for text in [
        "x".repeat(262145),
        "😀".repeat(65537),
        "bad\0text".into(),
        "bad\u{7f}".into(),
        "bad\u{85}".into(),
    ] {
        let mut changed = wire();
        changed["text"] = json!(text);
        error(
            &capture(
                app.clone(),
                request("POST", CREATE, Body::from(changed.to_string())),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    for (title, text) in [
        ("😀".repeat(64), "".into()),
        ("x".repeat(256), "\t中文😀\r\n".into()),
        ("Notes".into(), "x".repeat(262144)),
        ("Notes".into(), "文".repeat(87381) + "x"),
    ] {
        let mut changed = wire();
        changed["title"] = json!(title);
        changed["text"] = json!(text);
        let response = capture(
            app.clone(),
            request("POST", CREATE, Body::from(changed.to_string())),
        )
        .await;
        assert_eq!(response.status, StatusCode::CREATED);
        assert_eq!(response.json["artifact"]["title"], changed["title"]);
        assert_eq!(response.json["artifact"]["byte_length"], text.len());
    }
    assert_eq!(backend.calls(), 4);
}

#[tokio::test]
async fn text_publish_wire_budget_accepts_escaped_text_and_exact_two_mib_but_counts_actual_stream_bytes(
) {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let mut empty = wire();
    empty["text"] = json!("");
    let raw = empty.to_string();
    let escaped = raw.replace(
        "\"text\":\"\"",
        &format!("\"text\":\"{}\"", "\\u0078".repeat(262144)),
    );
    assert!(escaped.len() > 1024 * 1024 && escaped.len() < WIRE_LIMIT);
    assert_eq!(
        capture(app.clone(), request("POST", CREATE, Body::from(escaped)))
            .await
            .status,
        StatusCode::CREATED
    );
    let mut exact = raw.into_bytes();
    exact.resize(WIRE_LIMIT, b' ');
    assert_eq!(
        capture(
            app.clone(),
            request("POST", CREATE, Body::from(exact.clone()))
        )
        .await
        .status,
        StatusCode::CREATED
    );
    assert_eq!(backend.calls(), 2);
    for declared in [None, Some("1")] {
        let stream = futures_util::stream::iter([
            Ok::<_, std::io::Error>(Bytes::from(exact.clone())),
            Ok(Bytes::from_static(b" ")),
        ]);
        let mut req = request("POST", CREATE, Body::from_stream(stream));
        if let Some(length) = declared {
            req.headers_mut()
                .insert(header::CONTENT_LENGTH, length.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 2);
}

#[tokio::test]
async fn text_publish_media_encoding_trailers_and_stream_errors_fail_before_dispatch() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for content_type in [
        None,
        Some("text/plain"),
        Some("application/json;charset=utf-8"),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().remove(header::CONTENT_TYPE);
        if let Some(value) = content_type {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, value.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "not_attempted",
        );
    }
    let mut duplicate = request("POST", CREATE, untouched());
    duplicate.headers_mut().append(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    error(
        &capture(app.clone(), duplicate).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "not_attempted",
    );
    let mut encoded = request("POST", CREATE, untouched());
    encoded.headers_mut().insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    error(
        &capture(app.clone(), encoded).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let broken = Body::from_stream(futures_util::stream::iter([
        Ok(Bytes::from(wire().to_string())),
        Err(std::io::Error::other("synthetic after valid JSON")),
    ]));
    error(
        &capture(app.clone(), request("POST", CREATE, broken)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let frames = futures_util::stream::iter([
        Ok::<_, std::io::Error>(http_body::Frame::data(Bytes::from(wire().to_string()))),
        Ok(http_body::Frame::trailers(HeaderMap::new())),
    ]);
    error(
        &capture(
            app.clone(),
            request(
                "POST",
                CREATE,
                Body::new(http_body_util::StreamBody::new(frames)),
            ),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
    let mut valid = request("POST", CREATE, body());
    valid.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    assert_eq!(capture(app, valid).await.status, StatusCode::CREATED);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn text_publish_all_dispatched_errors_remain_unconfirmed_and_private() {
    for (outcome, status, code) in [
        (
            Outcome::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            Outcome::InvalidSession,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
        ),
        (Outcome::Denied, StatusCode::FORBIDDEN, "access_denied"),
        (Outcome::Conflict, StatusCode::CONFLICT, "conflict"),
        (
            Outcome::InvalidInput,
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
        (
            Outcome::Corrupt,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
    ] {
        let backend = Backend::new(outcome);
        error(
            &capture(app(backend.clone(), 4), request("POST", CREATE, body())).await,
            status,
            code,
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}
