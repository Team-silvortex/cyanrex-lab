use super::{fixture::Fixture, *};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
};

pub async fn within<F: std::future::Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("bounded actual Source, TLS transport and selected-file operation")
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
pub fn name(manifest: &FileTransferManifest, kind: &str, index: Option<u32>) -> String {
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    );
    match index {
        Some(index) => format!("{kind}-{hash}-{index}"),
        None => format!("{kind}-{hash}"),
    }
}
pub fn assert_requests(
    requests: &[server::ObservedRequest],
    manifest: &FileTransferManifest,
    seed: u128,
    token: &str,
    expected: &[Kind],
) {
    assert_eq!(
        requests.len(),
        expected.len(),
        "no extra GET, fallback or automatic retry"
    );
    for (request, kind) in requests.iter().zip(expected) {
        assert_eq!(request.method, "GET");
        assert!(request.kind == *kind, "exact actual request boundary");
        let (endpoint, index) = match kind {
            Kind::Manifest => ("/platform/v1/file-manifests", None),
            Kind::Chunk(index) => ("/platform/v1/file-chunks", Some(*index)),
        };
        assert_eq!(request.uri.path(), endpoint);
        let raw = request.uri.query().unwrap();
        assert!(!raw.contains('%') && !raw.contains('+'));
        let fields = raw
            .split('&')
            .map(|field| field.split_once('=').unwrap())
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), if index.is_some() { 7 } else { 6 });
        for (key, value) in [
            (
                "grant_id",
                sharing_fixture::recipient_grant(seed).to_string(),
            ),
            ("upload_id", publication_fixture::upload(seed).to_string()),
            ("file_id", manifest.file_id().to_string()),
            ("version_id", manifest.version_id().to_string()),
            ("byte_length", manifest.byte_length().to_string()),
            ("sha256", manifest.sha256().to_string()),
        ] {
            let matches = fields
                .iter()
                .filter(|(name, _)| *name == key)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].1, value);
        }
        if let Some(index) = index {
            let values = fields
                .iter()
                .filter(|(key, _)| *key == "chunk_index")
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0].1, index.to_string());
        }
        let cookie = format!("cyanrex_platform_session={token}");
        assert!(
            request.headers.get_all("cookie").iter().count() == 1
                && request
                    .headers
                    .get("cookie")
                    .is_some_and(|value| value.as_bytes() == cookie.as_bytes()),
            "exact dedicated Source Session cookie, with no credential in failure output"
        );
        for (key, value) in [
            ("x-cyanrex-platform-request", "1"),
            ("accept-encoding", "identity"),
            (
                "accept",
                if index.is_some() {
                    "application/octet-stream"
                } else {
                    "application/json"
                },
            ),
        ] {
            assert_eq!(request.headers.get_all(key).iter().count(), 1);
            assert_eq!(request.headers[key], value);
        }
        for key in [
            "origin",
            "authorization",
            "range",
            "if-range",
            "if-match",
            "if-none-match",
            "content-type",
        ] {
            assert!(!request.headers.contains_key(key));
        }
        assert!(request.body.is_empty());
    }
}
pub fn assert_progress(
    progress: &FileDeliveryProgress,
    phase: FileDeliveryPhase,
    manifest: &FileTransferManifest,
    chunks: u32,
    staged: u64,
    total: Option<u32>,
) {
    let value = progress
        .latest()
        .expect("actual admitted Source/TLS call reports one scalar phase");
    assert_eq!(value.phase(), phase);
    assert_eq!(value.staged_chunks(), chunks);
    assert_eq!(value.staged_bytes(), staged);
    assert_eq!(value.total_bytes(), manifest.byte_length());
    assert_eq!(value.total_chunks(), total);
}
pub async fn metadata_capacity(
    route: &Router,
    f: &Fixture,
    manifest: &FileTransferManifest,
    seed: u128,
) {
    use http_body_util::BodyExt;
    let path = format!("/platform/v1/file-manifests?grant_id={}&upload_id={}&file_id={}&version_id={}&byte_length={}&sha256={}",
        sharing_fixture::recipient_grant(seed), publication_fixture::upload(seed), manifest.file_id(),
        manifest.version_id(), manifest.byte_length(), manifest.sha256());
    let unread = Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, io::Error>>> {
            panic!("the actual unpolled metadata response holds its original HTTP slot");
        },
    ));
    let mut request = Request::builder()
        .method("GET")
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", format!("cyanrex_platform_session={}", f.token()))
        .body(unread)
        .unwrap();
    request
        .headers_mut()
        .get_mut("cookie")
        .unwrap()
        .set_sensitive(true);
    let response = within(route.clone().oneshot(request)).await.unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = within(response.into_body().collect())
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":"capacity_exhausted","outcome":"not_attempted"}})
    );
}
pub async fn ended(progress: &mut FileDeliveryProgress) {
    let last = progress.latest();
    for _ in 0..2 {
        match tokio::time::timeout(Duration::from_secs(3), progress.changed())
            .await
            .expect("actual caller drop closes its producer, not an event queue or a job kill")
        {
            Some(value) => assert_eq!(Some(value), last),
            None => {
                assert_eq!(progress.latest(), last);
                return;
            }
        }
    }
    panic!("one final unseen snapshot is the entire closed watch tail");
}
#[derive(Debug)]
pub struct Output {
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub links: u64,
    pub length: u64,
    pub same_visible_inode: bool,
    pub verified: Result<(), String>,
    pub actual_length: u64,
    pub pattern_matches: bool,
    pub digest: String,
}
pub fn observe_output(path: &Path, manifest: &FileTransferManifest) -> io::Result<Output> {
    let visible = fs::symlink_metadata(path)?;
    let mut file = fs::File::open(path)?;
    let original = file.metadata()?;
    let verified = verify_file(manifest, &mut file).map_err(|error| error.to_string());
    let mut reader = fs::File::open(path)?.take(manifest.byte_length() + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut offset = 0;
    let mut pattern_matches = true;
    loop {
        let amount = reader.read(&mut buffer)?;
        if amount == 0 {
            break;
        }
        for (index, value) in buffer[..amount].iter().enumerate() {
            pattern_matches &= *value == bytes::byte(offset + index as u64);
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    Ok(Output {
        device: original.dev(),
        inode: original.ino(),
        mode: original.mode(),
        uid: original.uid(),
        gid: original.gid(),
        links: original.nlink(),
        length: original.len(),
        same_visible_inode: (original.dev(), original.ino()) == (visible.dev(), visible.ino()),
        verified,
        actual_length: offset,
        pattern_matches,
        digest: format!("{:x}", hash.finalize()),
    })
}
pub fn assert_output(
    value: Output,
    manifest: &FileTransferManifest,
    parent: (u64, u64, u32, u32, u32),
) {
    value
        .verified
        .expect("independent ordered chunk SHA, whole SHA and exact EOF");
    assert!(value.same_visible_inode && value.pattern_matches);
    assert_eq!(value.mode & 0o7777, 0o400);
    assert_eq!(value.uid, parent.2);
    assert_eq!(value.gid, parent.3);
    assert_eq!(value.links, 1);
    assert_eq!(value.length, manifest.byte_length());
    assert_eq!(value.actual_length, manifest.byte_length());
    assert_eq!(value.digest, manifest.sha256().to_string());
}
pub async fn controlled(
    f: &Fixture,
    peer: &SourceTlsServer,
    root: &PathBuf,
    selected: &PathBuf,
    manifest: &FileTransferManifest,
    seed: u128,
    drop_handle: bool,
) -> Result<FileVersionRef, FileControlledSaveError> {
    let client = f.client(peer, root);
    let target = FileSaveTarget::open_user_directory(selected, fixture::CHOSEN_LEAF).unwrap();
    let expected = FileVersionRef::from_manifest(manifest);
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    let handle = if drop_handle {
        drop(handle);
        None
    } else {
        Some(handle)
    };
    let (observer, mut progress) = FileDeliveryObserver::channel();
    let result = within(client.fetch_granted_and_save_controlled(
        f.token(),
        sharing_fixture::recipient_grant(seed),
        publication_fixture::upload(seed),
        &expected,
        &target,
        observer,
        cancellation,
    ))
    .await
    .map(|saved| saved.file().clone());
    if let Some(handle) = handle {
        handle.cancel();
    }
    if result.is_ok() {
        assert_progress(
            &progress,
            FileDeliveryPhase::Complete,
            manifest,
            manifest.chunk_sha256().len() as u32,
            manifest.byte_length(),
            Some(manifest.chunk_sha256().len() as u32),
        );
    }
    ended(&mut progress).await;
    result
}
