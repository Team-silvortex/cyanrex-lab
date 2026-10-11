use super::*;

#[tokio::test]
async fn text_artifact_http_configuration_requires_canonical_origin_and_bounded_capacity() {
    let f = Offline::new().await;
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(TextArtifactHttpState::new(f.source.clone(), f.workspace.clone(), origin).is_ok());
    }
    for origin in [
        "",
        "null",
        "*",
        "https://*.example",
        "http://platform.example",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://a@platform.example",
        "https://platform.example?x=1",
        "https://platform.example#x",
    ] {
        assert!(
            matches!(
                TextArtifactHttpState::new(f.source.clone(), f.workspace.clone(), origin),
                Err(TaskContentHttpConfigError::InvalidOrigin)
            ),
            "{origin}"
        );
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TextArtifactHttpState::with_capacity(
                f.source.clone(),
                f.workspace.clone(),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
    }
    for capacity in [1, 4, 16] {
        assert!(TextArtifactHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
}

#[tokio::test]
async fn text_artifact_http_credentials_and_origin_do_not_accept_alternate_sources() {
    let f = Offline::new().await;
    for method in ["GET", "POST"] {
        let path = if method == "GET" {
            read_path()
        } else {
            TEXT_COLLECTION.into()
        };
        for mode in [
            "missing_marker",
            "marker_list",
            "marker_duplicate",
            "bad_origin",
            "duplicate_origin",
            "legacy",
            "duplicate_cookie",
            "cookie_headers",
            "uppercase_token",
            "nil_token",
            "authorization",
            "oversized_cookie",
        ] {
            let mut req = request(method, &path, TOKEN, Body::empty());
            let (status, code) = match mode {
                "missing_marker" => {
                    req.headers_mut().remove(MARKER);
                    (StatusCode::FORBIDDEN, "request_forbidden")
                }
                "marker_list" => {
                    req.headers_mut().insert(MARKER, "1, 1".parse().unwrap());
                    (StatusCode::FORBIDDEN, "request_forbidden")
                }
                "marker_duplicate" => {
                    req.headers_mut().append(MARKER, "1".parse().unwrap());
                    (StatusCode::FORBIDDEN, "request_forbidden")
                }
                "bad_origin" => {
                    req.headers_mut()
                        .insert("origin", "https://platform.example/".parse().unwrap());
                    (StatusCode::FORBIDDEN, "request_forbidden")
                }
                "duplicate_origin" => {
                    req.headers_mut().append("origin", ORIGIN.parse().unwrap());
                    (StatusCode::FORBIDDEN, "request_forbidden")
                }
                "legacy" => {
                    req.headers_mut()
                        .insert("cookie", "cyanrex_session=legacy".parse().unwrap());
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                "duplicate_cookie" => {
                    req.headers_mut().insert(
                        "cookie",
                        format!("{COOKIE}={TOKEN}; {COOKIE}={TOKEN}")
                            .parse()
                            .unwrap(),
                    );
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                "cookie_headers" => {
                    req.headers_mut()
                        .append("cookie", format!("{COOKIE}={TOKEN}").parse().unwrap());
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                "uppercase_token" => {
                    req.headers_mut().insert(
                        "cookie",
                        format!("{COOKIE}={}", TOKEN.to_uppercase())
                            .parse()
                            .unwrap(),
                    );
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                "nil_token" => {
                    req.headers_mut().insert(
                        "cookie",
                        format!("{COOKIE}={}", Uuid::nil()).parse().unwrap(),
                    );
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                "authorization" => {
                    req.headers_mut()
                        .insert("authorization", "Bearer synthetic".parse().unwrap());
                    (StatusCode::UNAUTHORIZED, "invalid_session")
                }
                _ => {
                    req.headers_mut().append(
                        "cookie",
                        format!("padding={}", "x".repeat(8192)).parse().unwrap(),
                    );
                    (StatusCode::PAYLOAD_TOO_LARGE, "request_too_large")
                }
            };
            offline_error(&f, req, status, code).await;
        }
    }
    let mut req = request("POST", TEXT_COLLECTION, TOKEN, valid_body(""));
    req.headers_mut().remove("origin");
    req.headers_mut()
        .insert("referer", format!("{ORIGIN}/editor").parse().unwrap());
    offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
    let mut req = request("GET", &read_path(), TOKEN, Body::empty());
    req.headers_mut().remove("origin");
    req.headers_mut()
        .append("cookie", "ordinary=opaque".parse().unwrap());
    dispatched(f.app.clone(), req).await;
}

#[tokio::test]
async fn text_artifact_http_exact_paths_methods_and_all_responses_are_private() {
    let f = Offline::new().await;
    let path = read_path();
    for bad in [
        path.replace(TEXT_ARTIFACT, &TEXT_ARTIFACT.to_uppercase()),
        path.replace(TEXT_ARTIFACT, "00000000-0000-0000-0000-000000000000"),
        path.replace(TEXT_ARTIFACT, &format!("%35{}", &TEXT_ARTIFACT[1..])),
        path.replace(TEXT_REVISION, &TEXT_REVISION.to_uppercase()),
        path.replace(TEXT_REVISION, "not-a-uuid"),
        path.replace(TEXT_SHA, &TEXT_SHA.to_uppercase()),
        path.replace(TEXT_SHA, &"a".repeat(63)),
        path.replace(TEXT_SHA, &format!("%61{}", &TEXT_SHA[1..])),
        format!("{path}?token={TOKEN}"),
        format!("{path}?unused=1"),
    ] {
        offline_error(
            &f,
            request("GET", &bad, TOKEN, Body::empty()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for bad in [
        "/not-mounted".to_string(),
        format!("{TEXT_COLLECTION}/"),
        format!("{TEXT_COLLECTION}/{TEXT_ARTIFACT}"),
        format!("{path}/extra"),
        format!("{path}/"),
        path.replace("/revisions/", "/head/"),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(request("GET", &bad, TOKEN, Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(!response.headers().contains_key("allow"));
        headers(&response);
        error(
            f.app.clone(),
            request("GET", &bad, TOKEN, Body::empty()),
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        )
        .await;
    }
    for (route, allowed) in [(TEXT_COLLECTION, "POST"), (path.as_str(), "GET")] {
        for method in ["DELETE", "OPTIONS", "HEAD"] {
            let response = f
                .app
                .clone()
                .oneshot(request(method, route, TOKEN, Body::empty()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(response.headers()["allow"], allowed);
            headers(&response);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            if method == "HEAD" {
                assert!(bytes.is_empty());
            } else {
                assert_eq!(
                    serde_json::from_slice::<Value>(&bytes).unwrap(),
                    json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}})
                );
            }
        }
    }
}
