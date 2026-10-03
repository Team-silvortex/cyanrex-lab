use super::*;

fn body_that_must_not_be_polled() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("a rejected admission header must not poll the request body")
        },
    ))
}

#[tokio::test]
async fn platform_http_method_rejections_advertise_only_the_known_route_method() {
    let f = Offline::new().await;
    for (path, allowed) in [
        (COLLECTION.to_string(), "POST"),
        (format!("{COLLECTION}/{TASK}"), "GET"),
        (format!("{COLLECTION}/{TASK}/content"), "PUT"),
        (format!("{COLLECTION}/{TASK}/status"), "POST"),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(request("DELETE", &path, TOKEN, Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response
                .headers()
                .get("allow")
                .and_then(|value| value.to_str().ok()),
            Some(allowed)
        );
        headers(&response);
    }
}

#[tokio::test]
async fn platform_http_configuration_accepts_only_canonical_secure_or_loopback_origins() {
    let f = Offline::new().await;
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(
            TaskContentHttpState::new(f.source.clone(), f.workspace.clone(), origin).is_ok(),
            "{origin}"
        );
    }
    for origin in [
        "",
        "null",
        "*",
        "https://*.example",
        "https://*",
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
        "https://platform.example, https://other.example",
    ] {
        assert!(
            TaskContentHttpState::new(f.source.clone(), f.workspace.clone(), origin).is_err(),
            "{origin}"
        );
    }
}

#[tokio::test]
async fn platform_http_cookie_is_canonical_unique_across_all_headers_and_not_legacy_auth() {
    let f = Offline::new().await;
    let path = format!("{COLLECTION}/{TASK}");
    for cookie in [
        "".to_string(),
        "cyanrex_session=legacy".to_string(),
        format!("{COOKIE}=invalid"),
        format!("{COOKIE}={}", TOKEN.to_ascii_uppercase()),
        format!("{COOKIE}={}", TOKEN.replace('-', "")),
        format!("{COOKIE}=00000000-0000-0000-0000-000000000000"),
        format!("{COOKIE}=\"{TOKEN}\""),
        format!("{COOKIE}={TOKEN}; {COOKIE}={TOKEN}"),
    ] {
        let mut req = request("GET", &path, TOKEN, Body::empty());
        req.headers_mut().insert("cookie", cookie.parse().unwrap());
        offline_error(&f, req, StatusCode::UNAUTHORIZED, "invalid_session").await;
    }
    let mut req = request("GET", &path, TOKEN, Body::empty());
    req.headers_mut().append(
        "cookie",
        format!("other=x; {COOKIE}={TOKEN}").parse().unwrap(),
    );
    offline_error(&f, req, StatusCode::UNAUTHORIZED, "invalid_session").await;
    for auth in ["Bearer synthetic", "Basic synthetic", ""] {
        let mut req = request("GET", &path, TOKEN, Body::empty());
        req.headers_mut()
            .insert("authorization", auth.parse().unwrap());
        offline_error(&f, req, StatusCode::UNAUTHORIZED, "invalid_session").await;
    }
    let mut req = request("GET", &path, TOKEN, Body::empty());
    req.headers_mut().append(
        "cookie",
        format!("padding={}", "x".repeat(8192)).parse().unwrap(),
    );
    offline_error(&f, req, StatusCode::PAYLOAD_TOO_LARGE, "request_too_large").await;
    let mut req = request("GET", &path, TOKEN, Body::empty());
    req.headers_mut()
        .append("cookie", "unrelated=opaque".parse().unwrap());
    error(
        f.app.clone(),
        req,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn platform_http_marker_is_mandatory_exact_and_single_even_for_reads() {
    let f = Offline::new().await;
    for method in ["GET", "POST", "PUT"] {
        let path = match method {
            "GET" => format!("{COLLECTION}/{TASK}"),
            "PUT" => format!("{COLLECTION}/{TASK}/content"),
            _ => COLLECTION.into(),
        };
        for value in [
            None,
            Some(""),
            Some("true"),
            Some("01"),
            Some("1, 1"),
            Some(" 1"),
        ] {
            let mut req = request(method, &path, TOKEN, "{}");
            req.headers_mut().remove(MARKER);
            if let Some(value) = value {
                req.headers_mut().insert(MARKER, value.parse().unwrap());
            }
            offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        let mut req = request(method, &path, TOKEN, "{}");
        req.headers_mut().append(MARKER, "1".parse().unwrap());
        offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
    }
    let mut req = request("POST", COLLECTION, TOKEN, body_that_must_not_be_polled());
    req.headers_mut().remove(MARKER);
    offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
}

#[tokio::test]
async fn platform_http_origin_is_exact_without_referer_or_environment_fallback() {
    let f = Offline::new().await;
    for method in ["GET", "POST", "PUT"] {
        let path = match method {
            "GET" => format!("{COLLECTION}/{TASK}"),
            "PUT" => format!("{COLLECTION}/{TASK}/content"),
            _ => COLLECTION.into(),
        };
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
            let mut req = request(method, &path, TOKEN, "{}");
            req.headers_mut().insert("origin", origin.parse().unwrap());
            offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
        let mut duplicate = request(method, &path, TOKEN, "{}");
        duplicate
            .headers_mut()
            .append("origin", ORIGIN.parse().unwrap());
        offline_error(&f, duplicate, StatusCode::FORBIDDEN, "request_forbidden").await;
        if method != "GET" {
            let mut absent = request(method, &path, TOKEN, "{}");
            absent.headers_mut().remove("origin");
            absent
                .headers_mut()
                .insert("referer", format!("{ORIGIN}/somewhere").parse().unwrap());
            offline_error(&f, absent, StatusCode::FORBIDDEN, "request_forbidden").await;
        }
    }
    let mut read = request("GET", &format!("{COLLECTION}/{TASK}"), TOKEN, Body::empty());
    read.headers_mut().remove("origin");
    error(
        f.app.clone(),
        read,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    let mut req = request("POST", COLLECTION, TOKEN, body_that_must_not_be_polled());
    req.headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    offline_error(&f, req, StatusCode::FORBIDDEN, "request_forbidden").await;
}

#[tokio::test]
async fn platform_http_path_query_fallback_and_wrong_methods_never_reach_a_store() {
    let f = Offline::new().await;
    for path in [
        format!("{COLLECTION}/not-a-uuid"),
        format!("{COLLECTION}/{}", TASK.to_ascii_uppercase()),
        format!("{COLLECTION}/%65{}", &TASK[1..]),
        format!("{COLLECTION}/00000000-0000-0000-0000-000000000000"),
        format!("{COLLECTION}/{TASK}?token={TOKEN}"),
        format!("{COLLECTION}/{TASK}?unused=1"),
    ] {
        offline_error(
            &f,
            request("GET", &path, TOKEN, Body::empty()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for path in [
        "/not-mounted",
        "/platform/v1/tasks/x/unknown",
        "/platform/v1/tasks/",
    ] {
        let response = f
            .app
            .clone()
            .oneshot(request("GET", path, TOKEN, Body::empty()))
            .await
            .unwrap();
        assert!(!response.headers().contains_key("allow"));
        offline_error(
            &f,
            request("GET", path, TOKEN, Body::empty()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    for (method, path, allowed) in [
        ("DELETE", COLLECTION.to_string(), "POST"),
        ("GET", COLLECTION.to_string(), "POST"),
        ("POST", format!("{COLLECTION}/{TASK}"), "GET"),
        ("OPTIONS", COLLECTION.to_string(), "POST"),
        ("GET", format!("{COLLECTION}/{TASK}/content"), "PUT"),
        ("GET", format!("{COLLECTION}/{TASK}/status"), "POST"),
    ] {
        let response = f
            .app
            .clone()
            .oneshot(request(method, &path, TOKEN, Body::empty()))
            .await
            .unwrap();
        assert_eq!(
            response
                .headers()
                .get("allow")
                .and_then(|value| value.to_str().ok()),
            Some(allowed)
        );
        offline_error(
            &f,
            request(method, &path, TOKEN, Body::empty()),
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
        )
        .await;
    }
    let response = f
        .app
        .clone()
        .oneshot(request(
            "HEAD",
            &format!("{COLLECTION}/{TASK}"),
            TOKEN,
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()["allow"], "GET");
    headers(&response);
    assert!(response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .is_empty());
}
