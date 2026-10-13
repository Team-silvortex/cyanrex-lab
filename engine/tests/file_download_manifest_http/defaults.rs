use crate::{fixture::*, offline::*, support::*, *};

#[tokio::test]
async fn manifest_http_valid_six_selector_request_dispatches_to_explicit_closed_source() {
    let f = Offline::new().await;
    available(&f.app).await;
    let reversed = format!(
        "{ENDPOINT}?{}",
        uri()
            .split_once('?')
            .unwrap()
            .1
            .split('&')
            .rev()
            .collect::<Vec<_>>()
            .join("&")
    );
    let mut native = request(TOKEN, &reversed);
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

#[tokio::test]
async fn manifest_http_origin_and_capacity_configuration_is_inert_canonical_and_bounded() {
    let f = Offline::new().await;
    for origin in [
        ORIGIN,
        "https://127.0.0.1",
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(
            FileDownloadManifestHttpState::new(f.source.clone(), f.workspace.clone(), origin)
                .is_ok()
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
        "https://platform.example#x",
        "https://user@platform.example",
        " https://platform.example",
        "https://platform.example ",
    ] {
        assert_eq!(
            FileDownloadManifestHttpState::new(f.source.clone(), f.workspace.clone(), origin).err(),
            Some(TaskContentHttpConfigError::InvalidOrigin)
        );
    }
    for capacity in [0, 17, usize::MAX] {
        assert_eq!(
            FileDownloadManifestHttpState::with_capacity(
                f.source.clone(),
                f.workspace.clone(),
                ORIGIN,
                capacity
            )
            .err(),
            Some(TaskContentHttpConfigError::InvalidCapacity)
        );
    }
    for capacity in [1, 4, 16] {
        assert!(FileDownloadManifestHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
}

#[tokio::test]
async fn manifest_http_exact_path_and_get_methods_reject_without_reading_body() {
    let f = Offline::new().await;
    for uri in [
        "/platform/v1/file-manifests/",
        "/platform/v1/file-manifests/extra",
        "/platform/v1/file-chunks",
        "/platform/v1/files",
        "/platform/v1/File-manifests",
        "/platform/v1/%66ile-manifests",
    ] {
        rejected(
            &f,
            body_request(TOKEN, uri, unread()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"] {
        let mut request = body_request(TOKEN, &uri(), unread());
        *request.method_mut() = method.parse().unwrap();
        let response = f.app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()["allow"], "GET");
        if method == "HEAD" {
            headers(&response);
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
}
