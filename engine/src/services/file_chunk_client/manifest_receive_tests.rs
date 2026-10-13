use super::*;
use crate::services::file_chunk_client::download_test_support::manifest;
use http_body::{Body, Frame, SizeHint};
use std::{
    collections::VecDeque,
    io,
    marker::PhantomPinned,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
};

struct Frames {
    values: Mutex<VecDeque<Result<Frame<Bytes>, io::Error>>>,
    polls: Arc<AtomicUsize>,
    pending_after: bool,
    always_empty: bool,
    _pinned: PhantomPinned,
}
impl Frames {
    fn new(values: Vec<Result<Frame<Bytes>, io::Error>>) -> Self {
        Self {
            values: Mutex::new(values.into()),
            polls: Arc::new(AtomicUsize::new(0)),
            pending_after: false,
            always_empty: false,
            _pinned: PhantomPinned,
        }
    }
}
impl Body for Frames {
    type Data = Bytes;
    type Error = io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = self.as_ref().get_ref();
        this.polls.fetch_add(1, Ordering::SeqCst);
        if this.always_empty {
            return Poll::Ready(Some(Ok(Frame::data(Bytes::new()))));
        }
        if let Some(value) = this.values.lock().unwrap().pop_front() {
            return Poll::Ready(Some(value));
        }
        if this.pending_after {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
    fn is_end_stream(&self) -> bool {
        true
    } // Deliberately false producer evidence.
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(0)
    }
}

fn encoded() -> (FileTransferManifest, Vec<u8>) {
    let manifest = manifest(b"seventeen bytes!!");
    let bytes = serde_json::to_vec(&manifest).unwrap();
    (manifest, bytes)
}

async fn decode(
    body: Frames,
    length: usize,
    expected: &FileVersionRef,
) -> Result<FileTransferManifest, FileManifestClientError> {
    collect(
        body,
        length,
        expected,
        Duration::from_secs(1),
        Duration::from_secs(2),
        Instant::now(),
    )
    .await
}

#[tokio::test]
async fn manifest_frames_ignore_false_end_and_size_hints_and_require_observed_none() {
    let (manifest, bytes) = encoded();
    let expected = FileVersionRef::from_manifest(&manifest);
    let mut values = vec![Ok(Frame::data(Bytes::new()))];
    values.extend(
        bytes
            .chunks(19)
            .map(|part| Ok(Frame::data(Bytes::copy_from_slice(part)))),
    );
    let body = Frames::new(values);
    let polls = body.polls.clone();
    let frame_count = bytes.len().div_ceil(19) + 1;
    assert_eq!(
        decode(body, bytes.len(), &expected).await.unwrap(),
        manifest
    );
    assert_eq!(
        polls.load(Ordering::SeqCst),
        frame_count + 1,
        "actual None, not the hint, ends the response"
    );
}

#[tokio::test]
async fn manifest_full_length_never_hides_trailer_error_or_extra_data() {
    let (manifest, bytes) = encoded();
    let expected = FileVersionRef::from_manifest(&manifest);
    for trailing in [
        Ok(Frame::trailers(axum::http::HeaderMap::new())),
        Err(io::Error::other(
            "synthetic secret diagnostics must not escape",
        )),
        Ok(Frame::data(Bytes::from_static(b" "))),
    ] {
        let body = Frames::new(vec![
            Ok(Frame::data(Bytes::copy_from_slice(&bytes))),
            trailing,
        ]);
        assert_eq!(
            decode(body, bytes.len(), &expected).await,
            Err(FileManifestClientError::InvalidResponse)
        );
    }
}

#[tokio::test]
async fn manifest_short_oversize_and_strict_json_fail_before_any_file_work() {
    let (manifest, bytes) = encoded();
    let expected = FileVersionRef::from_manifest(&manifest);
    let full = || Frames::new(vec![Ok(Frame::data(Bytes::copy_from_slice(&bytes)))]);
    assert_eq!(
        decode(full(), bytes.len() - 1, &expected).await,
        Err(FileManifestClientError::InvalidResponse)
    );
    assert_eq!(
        decode(full(), bytes.len() + 1, &expected).await,
        Err(FileManifestClientError::InvalidResponse)
    );
    let text = std::str::from_utf8(&bytes).unwrap();
    for invalid in [
        text.replacen('{', "{\"schema_version\":1,", 1),
        text.replacen(
            "\"workspace\":{",
            "\"workspace\":{\"workspace_id\":\"00000000-0000-0000-0000-000000000002\",",
            1,
        ),
        text.replacen('{', "{\"unexpected\":1,", 1),
        format!("[{text}]"),
        format!("{text} {{}}"),
        "null".into(),
    ] {
        let length = invalid.len();
        let body = Frames::new(vec![Ok(Frame::data(Bytes::from(invalid)))]);
        assert_eq!(
            decode(body, length, &expected).await,
            Err(FileManifestClientError::InvalidResponse)
        );
    }
    let whitespace = format!("{text}\n\t ");
    let length = whitespace.len();
    let body = Frames::new(vec![Ok(Frame::data(Bytes::from(whitespace)))]);
    assert_eq!(decode(body, length, &expected).await.unwrap(), manifest);
}

#[tokio::test]
async fn manifest_full_version_matching_checks_scope_and_every_file_pin() {
    let (manifest, bytes) = encoded();
    let expected = FileVersionRef::from_manifest(&manifest);
    for (field, value) in [
        (
            "file_id",
            serde_json::json!("00000000-0000-0000-0000-000000000013"),
        ),
        (
            "version_id",
            serde_json::json!("00000000-0000-0000-0000-000000000014"),
        ),
        ("byte_length", serde_json::json!(18)),
        ("sha256", serde_json::json!("a".repeat(64))),
        (
            "workspace",
            serde_json::json!({"authority_id":"00000000-0000-0000-0000-000000000001", "workspace_id":"00000000-0000-0000-0000-000000000022"}),
        ),
    ] {
        let mut raw = serde_json::to_value(&expected).unwrap();
        raw[field] = value;
        let wrong: FileVersionRef = serde_json::from_value(raw).unwrap();
        let body = Frames::new(vec![Ok(Frame::data(Bytes::copy_from_slice(&bytes)))]);
        assert_eq!(
            decode(body, bytes.len(), &wrong).await,
            Err(FileManifestClientError::InvalidResponse),
            "{field}"
        );
    }
}

#[tokio::test]
async fn manifest_ready_empty_frames_yield_without_refreshing_idle_deadline() {
    let (manifest, bytes) = encoded();
    let mut body = Frames::new(Vec::new());
    body.always_empty = true;
    let polls = body.polls.clone();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        collect(
            body,
            bytes.len(),
            &FileVersionRef::from_manifest(&manifest),
            Duration::from_millis(50),
            Duration::from_secs(1),
            Instant::now(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(FileManifestClientError::TimedOut));
    assert!(polls.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn manifest_pending_after_complete_data_still_requires_eof_and_fixed_total() {
    let (manifest, bytes) = encoded();
    let mut body = Frames::new(vec![Ok(Frame::data(Bytes::copy_from_slice(&bytes)))]);
    body.pending_after = true;
    let polls = body.polls.clone();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        collect(
            body,
            bytes.len(),
            &FileVersionRef::from_manifest(&manifest),
            Duration::from_secs(1),
            Duration::from_millis(100),
            Instant::now(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(FileManifestClientError::TimedOut));
    assert!(
        polls.load(Ordering::SeqCst) >= 2,
        "full bytes were observed before real EOF remained pending"
    );
}

#[test]
fn manifest_deadline_checks_never_revive_expired_total_or_idle() {
    let now = Instant::now();
    assert_eq!(
        check_deadline(now - Duration::from_secs(1), now + Duration::from_secs(1)),
        Err(FileManifestClientError::TimedOut)
    );
    assert_eq!(
        check_deadline(now + Duration::from_secs(1), now - Duration::from_secs(1)),
        Err(FileManifestClientError::TimedOut)
    );
    assert_eq!(
        check_deadline(now + Duration::from_secs(1), now + Duration::from_secs(1)),
        Ok(())
    );
}

#[tokio::test]
async fn manifest_maximum_digest_array_is_metadata_only_and_collects_within_wire_budget() {
    use crate::models::collaboration::{MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_CHUNKS};
    let baseline = manifest(b"seventeen bytes!!");
    let manifest = FileTransferManifest::new(
        baseline.workspace(),
        baseline.file_id(),
        baseline.version_id(),
        "maximum-metadata.bin",
        MAX_FILE_TRANSFER_BYTES,
        vec!["a".repeat(64).parse().unwrap(); MAX_FILE_TRANSFER_CHUNKS],
        "b".repeat(64).parse().unwrap(),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    assert!(bytes.len() <= MAX_FILE_TRANSFER_MANIFEST_BYTES);
    let length = bytes.len();
    let expected = FileVersionRef::from_manifest(&manifest);
    let body = Frames::new(vec![Ok(Frame::data(Bytes::from(bytes)))]);
    assert_eq!(
        collect(
            body,
            length,
            &expected,
            Duration::from_secs(5),
            Duration::from_secs(10),
            Instant::now()
        )
        .await
        .unwrap(),
        manifest
    );
}

#[tokio::test]
async fn manifest_body_deadlines_keep_original_header_finish_without_resetting_at_collection() {
    let (manifest, bytes) = encoded();
    let expected = FileVersionRef::from_manifest(&manifest);
    let body = Frames::new(vec![Ok(Frame::data(Bytes::copy_from_slice(&bytes)))]);
    let polls = body.polls.clone();
    let started = Instant::now() - Duration::from_secs(3);
    assert_eq!(
        collect(
            body,
            bytes.len(),
            &expected,
            Duration::from_secs(1),
            Duration::from_secs(2),
            started
        )
        .await,
        Err(FileManifestClientError::TimedOut)
    );
    assert_eq!(
        polls.load(Ordering::SeqCst),
        0,
        "already expired head-derived budget cannot reopen at collection"
    );
}
