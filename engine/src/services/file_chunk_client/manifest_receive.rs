//! Bounded private metadata collection. Returned JSON never becomes a stored-byte authority.
use super::{FileManifestClientError, FileTransferManifest, FileVersionRef};
use crate::models::collaboration::MAX_FILE_TRANSFER_MANIFEST_BYTES;
use crate::services::file_transfer::FILE_TRANSFER_BUFFER_BYTES;
use axum::body::Bytes;
use std::{future::poll_fn, time::Duration};
use tokio::time::{timeout_at, Instant};

pub(super) async fn collect<B>(
    body: B,
    length: usize,
    expected: &FileVersionRef,
    idle: Duration,
    total: Duration,
    started: Instant,
) -> Result<FileTransferManifest, FileManifestClientError>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
{
    if length == 0 || length > MAX_FILE_TRANSFER_MANIFEST_BYTES {
        return Err(FileManifestClientError::InvalidResponse);
    }
    let total_deadline = started + total;
    let mut idle_deadline = started + idle;
    check_deadline(total_deadline, idle_deadline)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| FileManifestClientError::Unavailable)?;
    let mut body = Box::pin(body);
    loop {
        check_deadline(total_deadline, idle_deadline)?;
        let frame = timeout_at(
            total_deadline.min(idle_deadline),
            poll_fn(|cx| body.as_mut().poll_frame(cx)),
        )
        .await
        .map_err(|_| FileManifestClientError::TimedOut)?;
        check_deadline(total_deadline, idle_deadline)?;
        let Some(frame) = frame else { break };
        let data = frame
            .map_err(|_| FileManifestClientError::InvalidResponse)?
            .into_data()
            .map_err(|_| FileManifestClientError::InvalidResponse)?;
        if data.len() > length - bytes.len() {
            return Err(FileManifestClientError::InvalidResponse);
        }
        if data.is_empty() {
            tokio::task::yield_now().await;
            continue;
        }
        for part in data.chunks(FILE_TRANSFER_BUFFER_BYTES) {
            check_deadline(total_deadline, idle_deadline)?;
            bytes.extend_from_slice(part);
            check_deadline(total_deadline, idle_deadline)?;
            idle_deadline = Instant::now() + idle;
            tokio::task::yield_now().await;
        }
    }
    check_deadline(total_deadline, idle_deadline)?;
    if bytes.len() != length {
        return Err(FileManifestClientError::InvalidResponse);
    }
    // Parse the original bytes directly: Value would erase duplicate fields before strict decoding.
    // The fixed byte/array limits bound work; this synchronous parse is not a hard CPU deadline.
    let parsed = FileTransferManifest::parse_json(&bytes);
    check_deadline(total_deadline, idle_deadline)?;
    let manifest = parsed.map_err(|_| FileManifestClientError::InvalidResponse)?;
    let matches = &FileVersionRef::from_manifest(&manifest) == expected;
    check_deadline(total_deadline, idle_deadline)?;
    if !matches {
        return Err(FileManifestClientError::InvalidResponse);
    }
    check_deadline(total_deadline, idle_deadline)?;
    // Actual message None is required, even after expected length. Trailers/errors never disappear.
    // Returned manifests and caller-retained clones hold no receive capacity or file-worker lease.
    Ok(manifest)
}

fn check_deadline(total: Instant, idle: Instant) -> Result<(), FileManifestClientError> {
    let now = Instant::now();
    if now >= total || now >= idle {
        Err(FileManifestClientError::TimedOut)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "manifest_receive_tests.rs"]
mod tests;
