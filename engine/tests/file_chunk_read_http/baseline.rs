use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_returns_verified_private_bytes_for_current_exact_recipient() {
    let f = Fixture::ready(1).await;
    let typed = f.request();
    let file = typed.download.expected_file.clone();
    let digest = f.sharing.base.manifest.chunk(0).unwrap().sha256.to_string();
    let expected = (0..17).map(bytes::byte).collect::<Vec<_>>();
    let before = f.sharing.snapshot().await;
    let response = app(&f)
        .oneshot(request(
            &f.sharing.base.grants.auth.target_token,
            &path(&typed),
        ))
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let returned = response
        .into_body()
        .collect()
        .await
        .map(|body| body.to_bytes().to_vec())
        .map_err(|error| error.to_string());
    let after = f.sharing.snapshot().await;
    f.cleanup().await;

    assert_eq!(status,StatusCode::OK,"actual current exact recipient must receive the verified private chunk, not a placeholder metadata response");
    assert_eq!(
        returned.expect("the first binary frame must complete normally"),
        expected
    );
    assert_eq!(after,before,"HTTP may not mutate Auth/Registry/both grants/upload/journal/Published rows/OIDs or staged byte identity");
    for (name, value) in [
        ("content-type", "application/octet-stream".to_owned()),
        ("content-length", "17".to_owned()),
        (
            "content-disposition",
            "attachment; filename=\"chunk.bin\"".to_owned(),
        ),
        ("x-cyanrex-file-transfer-version", "1".to_owned()),
        ("x-cyanrex-file-id", file.file_id().to_string()),
        ("x-cyanrex-file-version-id", file.version_id().to_string()),
        ("x-cyanrex-chunk-index", "0".to_owned()),
        ("x-cyanrex-chunk-sha256", digest),
        ("x-cyanrex-file-sha256", file.sha256().to_string()),
        ("cache-control", "no-store".to_owned()),
        ("pragma", "no-cache".to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
    ] {
        assert_eq!(
            headers.get(name).and_then(|v| v.to_str().ok()),
            Some(value.as_str()),
            "fixed trusted header {name}"
        );
    }
    for name in [
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "content-range",
        "etag",
        "last-modified",
        "x-cyanrex-owner-id",
        "x-cyanrex-account-id",
        "x-cyanrex-source-key",
        "x-cyanrex-root-key",
        "x-cyanrex-journal-key",
    ] {
        assert!(
            !headers.contains_key(name),
            "private response must not expose {name}"
        );
    }
}
