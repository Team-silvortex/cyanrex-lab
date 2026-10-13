use crate::{fixture::*, *};
use std::{fs, future::Future, os::unix::fs::MetadataExt, path::Path, time::Duration};

pub async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("bounded actual native HTTPS delivery operation")
}
pub fn directory_policy(path: &Path) -> (u64, u64, u32, u32, u32) {
    let metadata = fs::metadata(path).unwrap();
    (
        metadata.dev(),
        metadata.ino(),
        metadata.uid(),
        metadata.gid(),
        metadata.mode() & 0o7777,
    )
}
pub fn snapshot(
    f: &Fixture,
    progress: &FileDeliveryProgress,
    phase: FileDeliveryPhase,
    chunks: u32,
    bytes: u64,
    total_chunks: Option<u32>,
) -> FileDeliverySnapshot {
    let value = progress
        .latest()
        .expect("real controlled HTTPS boundary has a scalar snapshot");
    assert_eq!(value.phase(), phase);
    assert_eq!(value.staged_chunks(), chunks);
    assert_eq!(value.staged_bytes(), bytes);
    assert_eq!(value.total_bytes(), f.manifest.byte_length());
    assert_eq!(value.total_chunks(), total_chunks);
    let text = format!("{value:?}");
    for hidden in [
        TOKEN.to_owned(),
        f.origin.clone(),
        grant().to_string(),
        upload().to_string(),
        f.manifest.file_id().to_string(),
        f.manifest.version_id().to_string(),
        f.manifest.sha256().to_string(),
        f.manifest.display_name().to_owned(),
        f.root.to_string_lossy().into_owned(),
        f.selected.to_string_lossy().into_owned(),
    ] {
        assert!(
            !text.contains(&hidden),
            "scalar view contains no credentials or local paths"
        );
    }
    value
}
pub async fn closed(progress: &mut FileDeliveryProgress, expected: Option<FileDeliverySnapshot>) {
    for _ in 0..2 {
        match tokio::time::timeout(Duration::from_secs(3), progress.changed())
            .await
            .expect("closed waiter must not leave a producer in the actual file job")
        {
            Some(value) => assert_eq!(Some(value), expected),
            None => {
                assert_eq!(progress.latest(), expected);
                return;
            }
        }
    }
    panic!("watch has at most one final unseen slot, not an event queue");
}
pub fn assert_request(
    request: &ObservedRequest,
    manifest: &FileTransferManifest,
    index: Option<u32>,
) {
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.uri.path(),
        if index.is_none() {
            "/platform/v1/file-manifests"
        } else {
            "/platform/v1/file-chunks"
        }
    );
    let raw = request.uri.query().unwrap();
    assert!(!raw.contains('%') && !raw.contains('+'));
    assert_eq!(raw.split('&').count(), if index.is_none() { 6 } else { 7 });
    let expected = ManifestQuery {
        grant_id: grant(),
        upload_id: upload(),
        file_id: manifest.file_id(),
        version_id: manifest.version_id(),
        byte_length: manifest.byte_length(),
        sha256: manifest.sha256().clone(),
    };
    match request.selected.as_ref().unwrap() {
        Selected::Manifest(query) => {
            assert!(index.is_none());
            assert_eq!(query, &expected);
        }
        Selected::Chunk(query) => {
            assert_eq!(Some(query.chunk_index), index);
            assert_eq!(
                ManifestQuery {
                    grant_id: query.grant_id,
                    upload_id: query.upload_id,
                    file_id: query.file_id,
                    version_id: query.version_id,
                    byte_length: query.byte_length,
                    sha256: query.sha256.clone()
                },
                expected
            );
        }
    }
    for (name, value) in [
        ("cookie", format!("cyanrex_platform_session={TOKEN}")),
        ("x-cyanrex-platform-request", "1".to_owned()),
        ("accept-encoding", "identity".to_owned()),
        (
            "accept",
            if index.is_none() {
                "application/json"
            } else {
                "application/octet-stream"
            }
            .to_owned(),
        ),
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
        assert!(!request.headers.contains_key(name));
    }
    assert!(request.body.is_empty());
}
