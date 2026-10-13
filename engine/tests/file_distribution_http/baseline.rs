use crate::{fixture::*, *};

#[tokio::test]
async fn distribution_http_routes_manifest_and_chunk_gets_to_one_explicit_closed_source_with_private_errors(
) {
    let (responses, root_before, root_after, before, after) = checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let root_before = bytes::snapshot(&f.root);
            let before = bytes::tree(&f.root);
            let mut responses = Vec::new();
            for index in [None, Some(0)] {
                responses.push(observe(&f.app, request(&f.path(index))).await);
            }
            (
                responses,
                root_before,
                bytes::snapshot(&f.root),
                before,
                bytes::tree(&f.root),
            )
        })
    })
    .await;
    // Setup, both requests, response collection and private-directory cleanup precede this Red oracle.
    assert_eq!(responses.len(), 2);
    for (index, response) in responses.into_iter().enumerate() {
        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE,
            "route {index} must dispatch to its existing closed Source, not disappear behind chunk fallback");
        for (name, value) in [
            ("content-type", "application/json"),
            ("cache-control", "no-store"),
            ("pragma", "no-cache"),
            ("x-content-type-options", "nosniff"),
        ] {
            assert_eq!(response.headers[name], value);
        }
        for name in [
            "set-cookie",
            "location",
            "retry-after",
            "access-control-allow-origin",
            "content-disposition",
            "content-range",
            "etag",
            "last-modified",
            "x-cyanrex-owner-id",
            "x-cyanrex-account-id",
            "x-cyanrex-source-key",
            "x-cyanrex-root-key",
            "x-cyanrex-journal-key",
            "x-cyanrex-file-transfer-version",
            "x-cyanrex-file-id",
            "x-cyanrex-file-version-id",
            "x-cyanrex-file-sha256",
            "x-cyanrex-chunk-index",
            "x-cyanrex-chunk-sha256",
        ] {
            assert!(
                !response.headers.contains_key(name),
                "private error exposes no {name}"
            );
        }
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&response.body).unwrap(),
            serde_json::json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}})
        );
    }
    assert!(before.is_empty());
    assert_eq!(after, before);
    assert_eq!(
        root_after, root_before,
        "neither route reads or repairs private bytes after a closed Source error"
    );
    assert_eq!(root_before.mode & 0o7777, 0o700);
}
