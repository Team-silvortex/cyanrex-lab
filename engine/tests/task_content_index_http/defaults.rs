use super::*;

#[tokio::test]
async fn index_http_valid_request_dispatches_to_closed_source() {
    let f = Offline::new().await;
    error(
        f.app,
        request("GET", COLLECTION, Body::empty()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn index_http_trusted_origin_and_capacity_configuration_are_bounded() {
    let f = Offline::new().await;
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(
            TaskContentIndexHttpState::new(f.source.clone(), f.workspace.clone(), origin).is_ok()
        );
    }
    for origin in [
        "",
        "null",
        "*",
        "https://*.example",
        "http://platform.example",
        "http://192.168.1.1",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://platform.example?q=a",
        "https://platform.example#fragment",
        "https://a@platform.example",
        " https://platform.example",
        "https://platform.example ",
    ] {
        assert!(
            matches!(
                TaskContentIndexHttpState::new(f.source.clone(), f.workspace.clone(), origin),
                Err(TaskContentHttpConfigError::InvalidOrigin)
            ),
            "{origin}"
        );
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TaskContentIndexHttpState::with_capacity(
                f.source.clone(),
                f.workspace.clone(),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
    }
    for capacity in [1, 4, 16] {
        assert!(TaskContentIndexHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
}

#[tokio::test]
async fn index_http_exact_route_methods_and_head_never_read_bodies() {
    let f = Offline::new().await;
    for path in [
        "/platform/v1/task-index/",
        "/platform/v1/task-index/extra",
        "/platform/v1/tasks",
        "/platform/v1/task%2dindex",
        "/platform/v1/catalog-tasks",
        "/unrelated",
    ] {
        rejected(
            &f,
            request("GET", path, unread_body()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"] {
        let response = f
            .app
            .clone()
            .oneshot(request(method, COLLECTION, unread_body()))
            .await
            .unwrap();
        headers(&response);
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()["allow"], "GET");
        if method == "HEAD" {
            assert_eq!(response.headers()["content-length"], "0");
            assert!(response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty());
        } else {
            assert_eq!(
                response_value(response, StatusCode::METHOD_NOT_ALLOWED).await,
                json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}})
            );
        }
    }
}

#[tokio::test]
async fn index_http_query_is_raw_canonical_optional_and_never_authority() {
    let f = Offline::new().await;
    for suffix in [
        "?".to_string(),
        "?limit=".into(),
        "?after=".into(),
        "?limit=0".into(),
        "?limit=33".into(),
        "?limit=01".into(),
        "?limit=+1".into(),
        "?limit=-1".into(),
        "?limit=1.0".into(),
        "?limit=%31".into(),
        "?limit=1=1".into(),
        "?limit=1&".into(),
        "?&limit=1".into(),
        "?limit=1&&after=x".into(),
        "?limit=1&limit=1".into(),
        "?limit".into(),
        "?Limit=1".into(),
        "?owner=teacher".into(),
        "?namespace=other".into(),
        "?definition_ref=sample".into(),
        "?token=secret".into(),
        "?%6cimit=1".into(),
        "?limit=%zz".into(),
        format!("?after={}", TASK.to_uppercase()),
        format!("?after={}", TASK.replace('-', "")),
        "?after=00000000-0000-0000-0000-000000000000".into(),
        format!("?after=%65{}", &TASK[1..]),
        format!("?after={TASK}&after={TASK}"),
        format!("?after={TASK}+"),
        format!("?limit={}", "1".repeat(129)),
    ] {
        rejected(
            &f,
            request("GET", &format!("{COLLECTION}{suffix}"), unread_body()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for suffix in [
        "".to_string(),
        "?limit=1".into(),
        "?limit=20".into(),
        "?limit=32".into(),
        format!("?after={TASK}"),
        format!("?after={TASK}&limit=32"),
        format!("?limit=1&after={TASK}"),
    ] {
        error(
            f.app.clone(),
            request("GET", &format!("{COLLECTION}{suffix}"), Body::empty()),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}

#[tokio::test]
async fn index_http_cookie_and_authorization_are_unique_canonical_and_not_legacy() {
    let f = Offline::new().await;
    for cookie in [
        "".to_string(),
        "cyanrex_session=legacy".into(),
        format!("{COOKIE}=invalid"),
        format!("{COOKIE}={}", TOKEN.to_uppercase()),
        format!("{COOKIE}={}", TOKEN.replace('-', "")),
        format!("{COOKIE}=00000000-0000-0000-0000-000000000000"),
        format!("{COOKIE}=\"{TOKEN}\""),
        format!("{COOKIE}={TOKEN}; {COOKIE}={TOKEN}"),
    ] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut().insert("cookie", cookie.parse().unwrap());
        rejected(&f, req, StatusCode::UNAUTHORIZED, "invalid_session").await;
    }
    let mut duplicate = request("GET", COLLECTION, unread_body());
    duplicate.headers_mut().append(
        "cookie",
        format!("other=x; {COOKIE}={TOKEN}").parse().unwrap(),
    );
    rejected(&f, duplicate, StatusCode::UNAUTHORIZED, "invalid_session").await;
    for authorization in ["Bearer synthetic", "Basic synthetic", ""] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut()
            .insert("authorization", authorization.parse().unwrap());
        rejected(&f, req, StatusCode::UNAUTHORIZED, "invalid_session").await;
    }
    let mut huge = request("GET", COLLECTION, unread_body());
    huge.headers_mut().append(
        "cookie",
        format!("padding={}", "x".repeat(8192)).parse().unwrap(),
    );
    rejected(&f, huge, StatusCode::PAYLOAD_TOO_LARGE, "request_too_large").await;
    let mut unrelated = request("GET", COLLECTION, Body::empty());
    unrelated
        .headers_mut()
        .append("cookie", "unrelated=opaque".parse().unwrap());
    error(
        f.app,
        unrelated,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn index_http_marker_and_supplied_origin_are_exact_but_get_can_omit_origin() {
    let f = Offline::new().await;
    for marker in [
        None,
        Some(""),
        Some("true"),
        Some("01"),
        Some("1, 1"),
        Some(" 1"),
    ] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut().remove(MARKER);
        if let Some(marker) = marker {
            req.headers_mut().insert(MARKER, marker.parse().unwrap());
        }
        rejected(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
    }
    let mut duplicate = request("GET", COLLECTION, unread_body());
    duplicate.headers_mut().append(MARKER, "1".parse().unwrap());
    rejected(&f, duplicate, StatusCode::FORBIDDEN, "request_forbidden").await;
    for origin in [
        "null",
        "https://evil.example",
        "https://platform.example/",
        "https://PLATFORM.example",
        "https://platform.example:443",
        "https://platform.example?x=1",
        "https://platform.example, https://evil.example",
        "",
    ] {
        let mut req = request("GET", COLLECTION, unread_body());
        req.headers_mut().insert("origin", origin.parse().unwrap());
        rejected(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
    }
    let mut duplicate = request("GET", COLLECTION, unread_body());
    duplicate
        .headers_mut()
        .append("origin", ORIGIN.parse().unwrap());
    rejected(&f, duplicate, StatusCode::FORBIDDEN, "request_forbidden").await;
    let mut absent = request("GET", COLLECTION, Body::empty());
    absent.headers_mut().remove("origin");
    error(
        f.app,
        absent,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn index_http_budget_is_independent_and_old_collections_do_not_become_lists() {
    let f = Offline::new().await;
    let route = build_task_content_index_router(f.state(1));
    let held = route
        .clone()
        .oneshot(request("GET", COLLECTION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::SERVICE_UNAVAILABLE);
    error(
        route.clone(),
        request("GET", COLLECTION, unread_body()),
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    )
    .await;
    let directory = input_fixture::private_directory();
    let artifact = f
        .source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let manual =
        SessionTaskContentWorkspace::new("private_tasks", scope(), artifact.clone()).unwrap();
    let catalogue = SessionCatalogTaskContentWorkspace::from_definitions(
        "private_tasks",
        scope(),
        artifact.clone(),
        &catalog(&values()),
    )
    .unwrap();
    let families = [
        (
            build_task_content_router(
                TaskContentHttpState::with_capacity(f.source.clone(), manual, ORIGIN, 1).unwrap(),
            ),
            "/platform/v1/tasks",
        ),
        (
            build_catalog_task_content_router(
                CatalogTaskContentHttpState::with_capacity(f.source.clone(), catalogue, ORIGIN, 1)
                    .unwrap(),
            ),
            "/platform/v1/catalog-tasks",
        ),
    ];
    for (old, collection) in families {
        error(
            old.clone(),
            request("GET", collection, unread_body()),
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        )
        .await;
        error(
            old.clone(),
            request("GET", &format!("{collection}?limit=1"), unread_body()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
        error(
            old,
            request("GET", &format!("{collection}/{TASK}"), Body::empty()),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
    let text = build_text_artifact_router(
        TextArtifactHttpState::with_capacity(f.source.clone(), artifact, ORIGIN, 1).unwrap(),
    );
    error(
        text,
        request(
            "GET",
            &format!(
                "/platform/v1/text-artifacts/{TASK}/revisions/{TASK}/{}",
                "a".repeat(64)
            ),
            Body::empty(),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    drop(held);
    available(route).await;
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    fs::remove_dir(directory).unwrap();
}
