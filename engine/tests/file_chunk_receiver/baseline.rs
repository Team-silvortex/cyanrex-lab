use crate::{fixture::*, *};
use std::{fs, os::unix::fs::MetadataExt};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_receiver_stages_actual_authorized_http_chunk_into_distinct_private_root() {
    let f = Fixture::ready().await;
    let manifest = f.server.sharing.base.manifest.clone();
    let expected_file = FileVersionRef::from_manifest(&manifest);
    let expected_digest = manifest.chunk(0).unwrap().sha256.clone();
    let expected_bytes = (0..17).map(bytes::byte).collect::<Vec<_>>();
    let name = legacy_name(&manifest, 0);
    let output = f.client_root.join(&name);
    let client_root = fs::metadata(&f.client_root).unwrap();
    let server_root = fs::metadata(&f.server.sharing.base.root).unwrap();
    let before = f.server.sharing.snapshot().await;
    let response = http_fixture::app(&f.server)
        .oneshot(http_fixture::request(
            &f.server.sharing.base.grants.auth.target_token,
            &http_fixture::path(&f.server.request()),
        ))
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let receiver = FileChunkReceiver::new(f.client_staging.clone());
    let received = receiver.receive_and_stage(&manifest, 0, response).await;
    let observed = observe_chunk(&output, &manifest, 0);
    let names = fs::read_dir(&f.client_root).map(|entries| {
        entries
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()
    });
    let after = f.server.sharing.snapshot().await;
    drop(receiver);
    f.cleanup().await;

    assert_eq!(
        status,
        StatusCode::OK,
        "the input must be real authorized binary output"
    );
    assert_eq!(headers.get("content-length").unwrap(), "17");
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/octet-stream"
    );
    let received =
        received.expect("verified actual HTTP bytes must be staged, not a metadata stub");
    assert_eq!(received.file(), &expected_file);
    assert_eq!(received.chunk_index(), 0);
    assert_eq!(received.byte_length(), 17);
    assert_eq!(received.chunk_sha256(), &expected_digest);
    assert_ne!(
        (client_root.dev(), client_root.ino()),
        (server_root.dev(), server_root.ino()),
        "reception must use a distinct client staging root"
    );
    assert_eq!(client_root.mode() & 0o777, 0o700);
    assert_eq!(
        names.unwrap().unwrap(),
        vec![std::ffi::OsString::from(name)]
    );
    let observed = observed.expect("independent reopen must find the actual staged chunk");
    assert_eq!(observed.mode & 0o777, 0o400);
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, 17);
    assert!(observed.same_path_inode);
    observed
        .verified
        .expect("independent verify_chunk must pass");
    assert_eq!(observed.bytes, expected_bytes);
    assert_eq!(after, before, "reception may not mutate server Auth/Registry/grants/upload/journal/Published rows/OIDs or original staged files");
}
