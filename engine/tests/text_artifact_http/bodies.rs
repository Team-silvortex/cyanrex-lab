use super::*;

#[tokio::test]
async fn text_artifact_http_large_text_uses_its_own_envelope_not_task_manifest_limit() {
    let f = Offline::new().await;
    let body = valid_body(&"x".repeat(70 * 1024));
    assert!(body.len() > 64 * 1024 && body.len() < 2 * 1024 * 1024);
    dispatched(f.app.clone(), request("POST", TEXT_COLLECTION, TOKEN, body)).await;
    let workspace =
        SessionTaskContentWorkspace::new("private_tasks", scope(), f.workspace.clone()).unwrap();
    let task_app = build_task_content_router(
        TaskContentHttpState::new(f.source.clone(), workspace, ORIGIN).unwrap(),
    );
    let task = http_fixture::create_body(id(), &manifest("Unchanged Task limit", vec![]));
    error(
        task_app,
        request(
            "POST",
            http_fixture::COLLECTION,
            TOKEN,
            format!("{task}{}", " ".repeat(70 * 1024)),
        ),
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    )
    .await;
}

#[tokio::test]
async fn text_artifact_http_create_json_is_strict_and_rejects_client_authority() {
    let f = Offline::new().await;
    let valid = valid_body("hello");
    let mut bad = vec![
        "null".to_owned(),
        "[]".into(),
        "{}".into(),
        "\"text\"".into(),
        format!("[1,\"{TEXT_ARTIFACT}\",\"{TEXT_REVISION}\",\"Private text\",\"hello\"]"),
        format!("\u{feff}{valid}"),
        format!("{valid} {{}}"),
    ];
    for version in [json!(0), json!(2), json!(1.0), json!("1"), json!(null)] {
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value["schema_version"] = version;
        bad.push(value.to_string());
    }
    for key in [
        "schema_version",
        "artifact_id",
        "revision_id",
        "title",
        "text",
    ] {
        bad.push(valid.replacen(
            &format!("\"{key}\":"),
            &format!("\"{key}\":null,\"{key}\":"),
            1,
        ));
    }
    for key in [
        "owner",
        "scope",
        "namespace",
        "path",
        "sha256",
        "kind",
        "media_type",
        "filename",
        "language",
        "expected_revision",
    ] {
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value[key] = json!("request must not choose authority or server metadata");
        bad.push(value.to_string());
    }
    for key in ["artifact_id", "revision_id"] {
        for identifier in [
            "not-local-editor-authority".into(),
            TEXT_ARTIFACT.to_uppercase(),
            TEXT_ARTIFACT.replace('-', ""),
            Uuid::nil().to_string(),
        ] {
            let mut value: Value = serde_json::from_str(&valid).unwrap();
            value[key] = json!(identifier);
            bad.push(value.to_string());
        }
    }
    for body in bad {
        offline_error(
            &f,
            request("POST", TEXT_COLLECTION, TOKEN, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    offline_error(
        &f,
        request("POST", TEXT_COLLECTION, TOKEN, vec![0xff, 0xfe]),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    for escape in ["\\ud800", "\\udfff"] {
        offline_error(
            &f,
            request(
                "POST",
                TEXT_COLLECTION,
                TOKEN,
                valid.replace("hello", escape),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn text_artifact_http_title_and_decoded_text_have_exact_profile_bounds() {
    let f = Offline::new().await;
    for title in [
        "".to_string(),
        " \t".into(),
        "x".repeat(257),
        "😀".repeat(65),
        "no\nlines".into(),
        "bad\u{0085}control".into(),
    ] {
        let body = create_body(id(), id(), &title, "");
        offline_error(
            &f,
            request("POST", TEXT_COLLECTION, TOKEN, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for title in [
        "x".repeat(256),
        "😀".repeat(64),
        " \u{202e}inert Unicode label ".into(),
    ] {
        dispatched(
            f.app.clone(),
            request(
                "POST",
                TEXT_COLLECTION,
                TOKEN,
                create_body(id(), id(), &title, ""),
            ),
        )
        .await;
    }
    for text in [
        "".to_string(),
        "\t中文😀\r\n".into(),
        "x".repeat(256 * 1024),
        "😀".repeat(64 * 1024),
    ] {
        dispatched(
            f.app.clone(),
            request("POST", TEXT_COLLECTION, TOKEN, valid_body(&text)),
        )
        .await;
    }
    for text in [
        "x".repeat(256 * 1024 + 1),
        "😀".repeat(64 * 1024 + 1),
        "bad\0text".into(),
        "bad\u{007f}".into(),
        "bad\u{0085}".into(),
    ] {
        offline_error(
            &f,
            request("POST", TEXT_COLLECTION, TOKEN, valid_body(&text)),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn text_artifact_http_wire_limit_counts_stream_bytes_and_allows_escaped_text() {
    let f = Offline::new().await;
    let escaped = valid_body("").replace(
        "\"text\":\"\"",
        &format!("\"text\":\"{}\"", "\\u0078".repeat(256 * 1024)),
    );
    assert!(escaped.len() > 1024 * 1024 && escaped.len() < 2 * 1024 * 1024);
    dispatched(
        f.app.clone(),
        request("POST", TEXT_COLLECTION, TOKEN, escaped),
    )
    .await;
    let valid = valid_body("");
    let exact = format!("{valid}{}", " ".repeat(2 * 1024 * 1024 - valid.len()));
    dispatched(
        f.app.clone(),
        request("POST", TEXT_COLLECTION, TOKEN, exact.clone()),
    )
    .await;
    for declared in [None, Some("1")] {
        let data = futures_util::stream::iter([
            Ok::<_, std::io::Error>(axum::body::Bytes::from(exact.clone())),
            Ok(axum::body::Bytes::from_static(b" ")),
        ]);
        let mut req = request("POST", TEXT_COLLECTION, TOKEN, Body::from_stream(data));
        if let Some(length) = declared {
            req.headers_mut()
                .insert("content-length", length.parse().unwrap());
        }
        offline_error(&f, req, StatusCode::PAYLOAD_TOO_LARGE, "request_too_large").await;
    }
}

#[tokio::test]
async fn text_artifact_http_media_encoding_and_get_body_are_strict() {
    let f = Offline::new().await;
    for method in ["GET", "POST"] {
        let path = if method == "GET" {
            read_path()
        } else {
            TEXT_COLLECTION.into()
        };
        for mode in ["duplicate", "text", "encoding"] {
            let mut req = request(method, &path, TOKEN, Body::empty());
            let status = match mode {
                "duplicate" => {
                    req.headers_mut()
                        .append("content-type", "application/json".parse().unwrap());
                    StatusCode::UNSUPPORTED_MEDIA_TYPE
                }
                "text" => {
                    req.headers_mut()
                        .insert("content-type", "text/plain".parse().unwrap());
                    StatusCode::UNSUPPORTED_MEDIA_TYPE
                }
                _ => {
                    req.headers_mut()
                        .insert("content-encoding", "identity".parse().unwrap());
                    StatusCode::BAD_REQUEST
                }
            };
            offline_error(
                &f,
                req,
                status,
                if mode == "encoding" {
                    "invalid_request"
                } else {
                    "unsupported_media_type"
                },
            )
            .await;
        }
    }
    let mut req = request("POST", TEXT_COLLECTION, TOKEN, valid_body(""));
    req.headers_mut().remove("content-type");
    offline_error(
        &f,
        req,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
    let mut req = request("POST", TEXT_COLLECTION, TOKEN, valid_body(""));
    req.headers_mut().insert(
        "content-type",
        "application/json; charset=utf-8".parse().unwrap(),
    );
    dispatched(f.app.clone(), req).await;
    let mut req = request("GET", &read_path(), TOKEN, Body::empty());
    req.headers_mut().remove("content-type");
    dispatched(f.app.clone(), req).await;
    for declared in [None, Some("0")] {
        let mut req = request("GET", &read_path(), TOKEN, "x");
        if let Some(length) = declared {
            req.headers_mut()
                .insert("content-length", length.parse().unwrap());
        }
        offline_error(&f, req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    // A swallowed trailer must not look like rejection of an otherwise empty JSON body.
    // The preceding DATA is valid and would reach the closed source if the trailer were ignored.
    let frames = futures_util::stream::iter([
        Ok::<_, std::io::Error>(http_body::Frame::data(axum::body::Bytes::from(valid_body(
            "",
        )))),
        Ok(http_body::Frame::trailers(axum::http::HeaderMap::new())),
    ]);
    offline_error(
        &f,
        request(
            "POST",
            TEXT_COLLECTION,
            TOKEN,
            Body::new(http_body_util::StreamBody::new(frames)),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}

#[tokio::test]
async fn text_artifact_http_body_deadline_covers_pending_and_ready_empty_frames() {
    let f = Offline::new().await;
    let slow = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    tokio::time::timeout(
        Duration::from_secs(3),
        offline_error(
            &f,
            request("POST", TEXT_COLLECTION, TOKEN, slow),
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
        ),
    )
    .await
    .expect("the two-second stream deadline applies");
    let started = std::time::Instant::now();
    let storm = futures_util::stream::poll_fn(move |_| {
        std::task::Poll::Ready(if started.elapsed() < Duration::from_millis(2200) {
            Some(Ok::<_, std::io::Error>(axum::body::Bytes::new()))
        } else {
            None
        })
    });
    tokio::time::timeout(
        Duration::from_secs(3),
        offline_error(
            &f,
            request("POST", TEXT_COLLECTION, TOKEN, Body::from_stream(storm)),
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
        ),
    )
    .await
    .expect("always-ready empty frames must yield to the deadline");
    // Dropping the stream error after valid DATA would incorrectly dispatch, not fail parsing.
    let broken = Body::from_stream(futures_util::stream::iter([
        Ok(axum::body::Bytes::from(valid_body(""))),
        Err(std::io::Error::other("synthetic text request failure")),
    ]));
    offline_error(
        &f,
        request("POST", TEXT_COLLECTION, TOKEN, broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}
