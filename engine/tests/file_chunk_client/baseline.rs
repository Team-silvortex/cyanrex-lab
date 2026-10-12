use crate::{fixture::*, *};
use std::{collections::BTreeMap, fs, os::unix::fs::MetadataExt};

#[tokio::test]
async fn loopback_client_fetches_exact_request_and_stages_real_verified_seventeen_bytes() {
    let f = Fixture::new().await;
    let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
    let expected_file = FileVersionRef::from_manifest(&f.manifest);
    let expected_chunk = f.manifest.chunk(0).unwrap().sha256;
    let expected_bytes = (0..17).map(bytes::byte).collect::<Vec<_>>();
    let expected_name = name(&f.manifest);
    let expected_query = BTreeMap::from([
        ("grant_id".to_owned(), grant().to_string()),
        ("upload_id".to_owned(), upload().to_string()),
        ("file_id".to_owned(), f.manifest.file_id().to_string()),
        ("version_id".to_owned(), f.manifest.version_id().to_string()),
        ("byte_length".to_owned(), "17".to_owned()),
        ("sha256".to_owned(), f.manifest.sha256().to_string()),
        ("chunk_index".to_owned(), "0".to_owned()),
    ]);
    let result = client
        .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0)
        .await;
    let observed = saved(&f.root.join(&expected_name), &f.manifest);
    let entries = fs::read_dir(&f.root).map(|entries| {
        entries
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<Vec<_>, _>>()
    });
    let root_mode = fs::metadata(&f.root).map(|m| m.mode() & 0o777);
    let requests = f.observed();
    drop(client);
    f.cleanup().await;

    let received = result.expect("the client must make the actual loopback request and stage verified bytes, not return a placeholder");
    assert_eq!(received.file(), &expected_file);
    assert_eq!(received.chunk_index(), 0);
    assert_eq!(received.byte_length(), 17);
    assert_eq!(received.chunk_sha256(), &expected_chunk);
    assert_eq!(root_mode.unwrap(), 0o700);
    assert_eq!(
        entries.unwrap().unwrap(),
        vec![std::ffi::OsString::from(expected_name)]
    );
    let observed = observed.expect("an independent descriptor must find the saved private chunk");
    assert_eq!(observed.mode & 0o777, 0o400);
    assert_eq!(observed.length, 17);
    assert_eq!(observed.links, 1);
    assert!(observed.same_inode);
    observed
        .verified
        .expect("independent exact EOF and SHA-256 check");
    assert_eq!(observed.bytes, expected_bytes);
    assert_eq!(requests.len(), 1, "no hidden retries or extra requests");
    let request = &requests[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.uri.path(), "/platform/v1/file-chunks");
    let raw = request.uri.query().unwrap();
    assert!(!raw.contains('%') && !raw.contains('+'));
    let fields = raw
        .split('&')
        .map(|pair| pair.split_once('=').unwrap())
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 7);
    let actual = fields
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected_query);
    for (name, value) in [
        ("cookie", format!("cyanrex_platform_session={TOKEN}")),
        ("x-cyanrex-platform-request", "1".to_owned()),
        ("accept-encoding", "identity".to_owned()),
        ("accept", "application/octet-stream".to_owned()),
    ] {
        assert_eq!(request.headers.get_all(name).iter().count(), 1);
        assert_eq!(request.headers[name], value);
    }
    for name in [
        "origin",
        "authorization",
        "content-type",
        "range",
        "if-range",
        "if-match",
        "if-none-match",
    ] {
        assert!(
            !request.headers.contains_key(name),
            "the client must not invent {name}"
        );
    }
    assert!(request.body.is_empty());
}
