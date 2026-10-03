use super::*;

fn valid_create() -> String {
    create_body(TASK.parse().unwrap(), &manifest("A text-free task", vec![]))
}

#[tokio::test]
async fn platform_http_json_root_and_create_identifiers_are_strict_not_local_draft_imports() {
    let f = Offline::new().await;
    let valid = valid_create();
    let mut cases = vec![
        "null".into(),
        "[]".into(),
        "{}".into(),
        "\"text\"".into(),
        format!("[1,\"{TASK}\",{{\"schema_version\":1,\"title\":\"T\",\"payload\":[]}}]"),
        format!("{valid} {{}}"),
        format!("\u{feff}{valid}"),
        valid.replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        ),
        format!("{{\"extra\":true,{}", &valid[1..]),
        json!({"format":"cyanrex.task-draft","version":1,"title":"Local","payload":[]}).to_string(),
    ];
    for version in [json!(0), json!(2), json!("1"), json!(null), json!(1.0)] {
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value["schema_version"] = version;
        cases.push(value.to_string());
    }
    for task_id in [
        TASK.to_uppercase(),
        TASK.replace('-', ""),
        "00000000-0000-0000-0000-000000000000".into(),
        "local-editor-id".into(),
    ] {
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value["task_id"] = json!(task_id);
        cases.push(value.to_string());
    }
    for body in cases {
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    offline_error(
        &f,
        request("POST", COLLECTION, TOKEN, vec![0xff, 0xfe]),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    error(
        f.app.clone(),
        request("POST", COLLECTION, TOKEN, valid),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn platform_http_nested_manifest_is_object_only_exact_and_byte_bounded() {
    let f = Offline::new().await;
    let artifact = json!({"workspace":scope(),"artifact_id":Uuid::new_v4(),"revision_id":Uuid::new_v4(),"sha256":"a".repeat(64)});
    let binding =
        json!({"kind":"text","filename":"a.txt","language":"future.language","artifact":artifact});
    let base = json!({"schema_version":1,"title":"Example","payload":[binding]});
    let mut bad = vec![
        json!([1, "Example", []]),
        json!({"schema_version":1,"title":" ","payload":[]}),
        json!({"schema_version":1,"title":"Example","payload":[],"text":"inline"}),
    ];
    for (key, value) in [
        ("kind", json!({"text":null})),
        ("kind", json!("binary")),
        ("filename", json!("../escape")),
        ("language", json!("Rust")),
        ("text", json!("inline")),
        ("revision", json!(1)),
    ] {
        let mut value_manifest = base.clone();
        value_manifest["payload"][0][key] = value;
        bad.push(value_manifest);
    }
    let mut value = base.clone();
    value["payload"][0] = json!(["text", "a.txt", "plaintext", artifact]);
    bad.push(value);
    let mut value = base.clone();
    value["payload"][0]["artifact"] =
        json!([scope(), Uuid::new_v4(), Uuid::new_v4(), "a".repeat(64)]);
    bad.push(value);
    let mut value = base.clone();
    value["payload"][0]["artifact"]["workspace"] =
        json!([scope().authority_id, scope().workspace_id]);
    bad.push(value);
    let mut value = base.clone();
    value["payload"] = json!([binding.clone(), binding.clone()]);
    bad.push(value);
    let mut value = base.clone();
    value["title"] = json!("中".repeat(86));
    bad.push(value);
    for manifest in bad {
        let body = json!({"schema_version":1,"task_id":TASK,"manifest":manifest}).to_string();
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    let body = json!({"schema_version":1,"task_id":TASK,"manifest":base}).to_string();
    for key in ["title", "kind", "sha256", "workspace_id"] {
        let needle = format!("\"{key}\":");
        let duplicate = body.replacen(&needle, &format!("\"{key}\":null,{needle}"), 1);
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, duplicate),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn platform_http_replace_and_status_require_safe_revision_and_string_status() {
    let f = Offline::new().await;
    for (method, suffix, field, valid) in [
        (
            "PUT",
            "content",
            "manifest",
            json!(manifest("Replacement", vec![])),
        ),
        ("POST", "status", "status", json!("ready")),
    ] {
        let path = format!("{COLLECTION}/{TASK}/{suffix}");
        let body = json!({"schema_version":1,"expected_revision":1,field:valid});
        for revision in [
            json!(0),
            json!(-1),
            json!(1.5),
            json!(1.0),
            json!(9007199254740992_u64),
            json!("1"),
            json!(null),
        ] {
            let mut value = body.clone();
            value["expected_revision"] = revision;
            offline_error(
                &f,
                request(method, &path, TOKEN, value.to_string()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
        for value in [
            json!([1, 1, valid]),
            json!({"schema_version":1,field:valid}),
            json!({"schema_version":2,"expected_revision":1,field:valid}),
            json!({"schema_version":1,"expected_revision":1,field:valid,"owner":scope()}),
        ] {
            offline_error(
                &f,
                request(method, &path, TOKEN, value.to_string()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
        let duplicate = body.to_string().replacen(
            "\"expected_revision\":1",
            "\"expected_revision\":1,\"expected_revision\":1",
            1,
        );
        offline_error(
            &f,
            request(method, &path, TOKEN, duplicate),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for status in [
        json!({"ready":null}),
        json!(["ready"]),
        json!("READY"),
        json!("execute"),
        json!(null),
    ] {
        offline_error(
            &f,
            request(
                "POST",
                &format!("{COLLECTION}/{TASK}/status"),
                TOKEN,
                json!({"schema_version":1,"expected_revision":1,"status":status}).to_string(),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn platform_http_media_encoding_and_get_actual_body_are_checked_before_storage() {
    let f = Offline::new().await;
    for content_type in [
        None,
        Some("text/plain"),
        Some("application/json; charset=latin1"),
        Some("application/json, application/json"),
    ] {
        let mut req = request("POST", COLLECTION, TOKEN, valid_create());
        req.headers_mut().remove("content-type");
        if let Some(value) = content_type {
            req.headers_mut()
                .insert("content-type", value.parse().unwrap());
        }
        offline_error(
            &f,
            req,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        )
        .await;
    }
    let mut req = request("POST", COLLECTION, TOKEN, valid_create());
    req.headers_mut()
        .append("content-type", "application/json".parse().unwrap());
    offline_error(
        &f,
        req,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
    for encoding in ["gzip", "identity"] {
        let mut req = request("POST", COLLECTION, TOKEN, valid_create());
        req.headers_mut()
            .insert("content-encoding", encoding.parse().unwrap());
        offline_error(&f, req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    let mut req = request("POST", COLLECTION, TOKEN, valid_create());
    req.headers_mut().insert(
        "content-type",
        "application/json; charset=utf-8".parse().unwrap(),
    );
    error(
        f.app.clone(),
        req,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    for declared_empty in [false, true] {
        let mut req = request("GET", &format!("{COLLECTION}/{TASK}"), TOKEN, "x");
        if declared_empty {
            req.headers_mut()
                .insert("content-length", "0".parse().unwrap());
        }
        offline_error(&f, req, StatusCode::BAD_REQUEST, "invalid_request").await;
    }
    let data = futures_util::stream::iter([Ok::<_, std::io::Error>(
        axum::body::Bytes::from_static(b"x"),
    )]);
    offline_error(
        &f,
        request(
            "GET",
            &format!("{COLLECTION}/{TASK}"),
            TOKEN,
            Body::from_stream(data),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}

#[tokio::test]
async fn platform_http_body_limit_counts_actual_stream_bytes_and_accepts_the_exact_limit() {
    let f = Offline::new().await;
    let valid = valid_create();
    let edge = format!("{valid}{}", " ".repeat(65536 - valid.len()));
    error(
        f.app.clone(),
        request("POST", COLLECTION, TOKEN, edge.clone()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    for misleading_length in [None, Some("1")] {
        let chunks = futures_util::stream::iter([
            Ok::<_, std::io::Error>(axum::body::Bytes::from(edge.clone())),
            Ok(axum::body::Bytes::from_static(b" ")),
        ]);
        let mut req = request("POST", COLLECTION, TOKEN, Body::from_stream(chunks));
        if let Some(value) = misleading_length {
            req.headers_mut()
                .insert("content-length", value.parse().unwrap());
        }
        offline_error(&f, req, StatusCode::PAYLOAD_TOO_LARGE, "request_too_large").await;
    }
}

#[tokio::test]
async fn platform_http_slow_or_broken_body_cannot_start_a_session_command() {
    let f = Offline::new().await;
    let slow = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    tokio::time::timeout(
        Duration::from_secs(3),
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, slow),
            StatusCode::REQUEST_TIMEOUT,
            "request_timeout",
        ),
    )
    .await
    .expect("two-second stream deadline was not enforced");
    let broken = Body::from_stream(futures_util::stream::iter([Err::<axum::body::Bytes, _>(
        std::io::Error::other("synthetic body read failure"),
    )]));
    offline_error(
        &f,
        request("POST", COLLECTION, TOKEN, broken),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
}

#[tokio::test]
async fn platform_http_always_ready_empty_frames_cannot_starve_the_body_deadline() {
    let f = Offline::new().await;
    let started = std::time::Instant::now();
    let storm = futures_util::stream::poll_fn(move |_| {
        std::task::Poll::Ready(if started.elapsed() < Duration::from_millis(2200) {
            Some(Ok::<_, std::io::Error>(axum::body::Bytes::new()))
        } else {
            None
        })
    });
    offline_error(
        &f,
        request("POST", COLLECTION, TOKEN, Body::from_stream(storm)),
        StatusCode::REQUEST_TIMEOUT,
        "request_timeout",
    )
    .await;
}

#[tokio::test]
async fn platform_http_get_rejects_duplicate_or_unsupported_declared_media_types() {
    let f = Offline::new().await;
    let path = format!("{COLLECTION}/{TASK}");
    let mut duplicate = request("GET", &path, TOKEN, Body::empty());
    duplicate
        .headers_mut()
        .append("content-type", "application/json".parse().unwrap());
    offline_error(
        &f,
        duplicate,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
    let mut non_json = request("GET", &path, TOKEN, Body::empty());
    non_json
        .headers_mut()
        .insert("content-type", "text/plain".parse().unwrap());
    offline_error(
        &f,
        non_json,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
}
