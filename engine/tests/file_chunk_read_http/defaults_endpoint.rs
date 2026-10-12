use crate::{fixture::*, offline::*, support::*, *};
#[tokio::test]
async fn chunk_http_exact_endpoint_and_all_raw_selectors_are_strict() {
    let f = Offline::new().await;
    let good = uri();
    for p in [
        "/platform/v1/file-chunks/",
        "/platform/v1/file-chunks/0",
        "/platform/v1/files",
        "/platform/v1/File-chunks",
        "/platform/v1/%66ile-chunks",
    ] {
        rejected(
            &f,
            request_body(TOKEN, p, unread()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    for method in ["POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"] {
        let response = f
            .app
            .clone()
            .oneshot(request_method(method, &good, unread()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        headers(&response);
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
            error_response(
                response,
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "not_attempted",
            )
            .await;
        }
    }
    let pairs = good
        .split_once('?')
        .unwrap()
        .1
        .split('&')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for i in 0..pairs.len() {
        let missing = format!(
            "{ENDPOINT}?{}",
            pairs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, p)| p.as_str())
                .collect::<Vec<_>>()
                .join("&")
        );
        rejected(
            &f,
            request_body(TOKEN, &missing, unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
        let duplicate = format!("{good}&{}", pairs[i]);
        rejected(
            &f,
            request_body(TOKEN, &duplicate, unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
        let key = pairs[i].split_once('=').unwrap().0;
        let mut modified = pairs.clone();
        modified[i] = format!("{key}=");
        rejected(
            &f,
            request_body(
                TOKEN,
                &format!("{ENDPOINT}?{}", modified.join("&")),
                unread(),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for extra in [
        "owner=teacher",
        "namespace=private",
        "workspace=x",
        "path=/tmp/x",
        "range=0-1",
        "unknown=1",
        "",
        "grant_id=x=y",
    ] {
        rejected(
            &f,
            request_body(TOKEN, &format!("{good}&{extra}"), unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for p in [
        ENDPOINT.to_owned(),
        format!("{ENDPOINT}?"),
        format!("{good}&x={}", "x".repeat(512)),
    ] {
        rejected(
            &f,
            request_body(TOKEN, &p, unread()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for key in ["grant_id", "upload_id", "file_id", "version_id"] {
        for bad in [
            Uuid::nil().to_string(),
            "ABCD1234-1234-4234-8234-123456789abc".into(),
            Uuid::new_v4().simple().to_string(),
            "%61".into(),
            "+".into(),
            "%zz".into(),
        ] {
            let replaced = pairs
                .iter()
                .map(|p| {
                    if p.starts_with(&format!("{key}=")) {
                        format!("{key}={bad}")
                    } else {
                        p.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            rejected(
                &f,
                request_body(TOKEN, &format!("{ENDPOINT}?{replaced}"), unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
    }
    for (key, values) in [
        (
            "byte_length",
            vec![
                "017",
                "-1",
                "+17",
                "17.0",
                "68719476737",
                "18446744073709551616",
            ],
        ),
        (
            "chunk_index",
            vec!["00", "01", "-1", "+0", "0.0", "16384", "4294967296"],
        ),
        (
            "sha256",
            vec![
                "BAD",
                "Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "%61",
            ],
        ),
    ] {
        for bad in values {
            let q = pairs
                .iter()
                .map(|p| {
                    if p.starts_with(&format!("{key}=")) {
                        format!("{key}={bad}")
                    } else {
                        p.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            rejected(
                &f,
                request_body(TOKEN, &format!("{ENDPOINT}?{q}"), unread()),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
        }
    }
    let reversed = format!(
        "{ENDPOINT}?{}",
        pairs.into_iter().rev().collect::<Vec<_>>().join("&")
    );
    error(
        f.app.clone(),
        request(TOKEN, &reversed),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    available(f.app).await;
}
