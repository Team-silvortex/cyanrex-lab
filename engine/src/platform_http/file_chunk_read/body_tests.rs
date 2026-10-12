use super::*;
use http_body_util::BodyExt;
use std::sync::Arc;
use tokio::sync::Semaphore;

// Synthetic Bytes owner tests only the private body/lease mechanism. It cannot manufacture the
// public Source proof; real verified bytes with both Source and HTTP leases are integration cases.
struct SyntheticOwner {
    bytes: &'static [u8],
    _source: OwnedSemaphorePermit,
    _http: OwnedSemaphorePermit,
}
impl AsRef<[u8]> for SyntheticOwner {
    fn as_ref(&self) -> &[u8] {
        self.bytes
    }
}

fn synthetic(deadline: Instant) -> (ChunkBody, Arc<Semaphore>, Arc<Semaphore>) {
    let source = Arc::new(Semaphore::new(1));
    let http = Arc::new(Semaphore::new(1));
    let data = Bytes::from_owner(SyntheticOwner {
        bytes: b"private verified bytes",
        _source: source.clone().try_acquire_owned().unwrap(),
        _http: http.clone().try_acquire_owned().unwrap(),
    });
    (
        ChunkBody {
            data: Some(data),
            deadline,
        },
        source,
        http,
    )
}

#[tokio::test]
async fn data_clones_and_slices_retain_both_leases_after_eof_and_body_drop() {
    let (mut body, source, http) = synthetic(first_frame_deadline());
    assert_eq!(source.available_permits(), 0);
    assert_eq!(http.available_permits(), 0);
    assert_eq!(body.size_hint().exact(), Some(22));
    assert!(!body.is_end_stream());
    let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(&data[..], b"private verified bytes");
    assert!(body.is_end_stream());
    assert!(body.frame().await.is_none());
    assert_eq!(body.size_hint().exact(), Some(0));
    let clone = data.clone();
    let slice = clone.slice(0..7);
    drop(body);
    drop(data);
    drop(clone);
    assert_eq!(&slice[..], b"private");
    assert_eq!(source.available_permits(), 0);
    assert_eq!(http.available_permits(), 0);
    drop(slice);
    assert_eq!(source.available_permits(), 1);
    assert_eq!(http.available_permits(), 1);
}

#[test]
fn unpolled_body_retains_expired_owner_until_explicit_drop() {
    let deadline = Instant::now() - Duration::from_secs(1);
    let (body, source, http) = synthetic(deadline);
    assert!(!body.is_end_stream());
    assert_eq!(source.available_permits(), 0);
    assert_eq!(http.available_permits(), 0);
    drop(body);
    assert_eq!(source.available_permits(), 1);
    assert_eq!(http.available_permits(), 1);
}

#[tokio::test]
async fn late_first_poll_yields_one_static_error_and_never_a_data_prefix() {
    let deadline = Instant::now() - Duration::from_secs(1);
    let (mut body, source, http) = synthetic(deadline);
    let error = body.frame().await.unwrap().unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(error.to_string(), "binary first frame deadline exceeded");
    assert_eq!(source.available_permits(), 1);
    assert_eq!(http.available_permits(), 1);
    assert!(body.is_end_stream());
    assert!(body.frame().await.is_none());
    assert!(body.frame().await.is_none());
}

#[tokio::test]
async fn axum_body_keeps_transferred_owner_until_last_bytes_drop() {
    let (inner, source, http) = synthetic(first_frame_deadline());
    let mut body = Body::new(inner);
    let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(body.is_end_stream());
    assert!(body.frame().await.is_none());
    drop(body);
    assert_eq!(source.available_permits(), 0);
    assert_eq!(http.available_permits(), 0);
    drop(data);
    assert_eq!(source.available_permits(), 1);
    assert_eq!(http.available_permits(), 1);
}
