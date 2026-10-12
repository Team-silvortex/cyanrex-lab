//! Private bounded frame collection, never a network client or an authenticated response proof.
use super::{Bytes, FileChunkReceiveError};
use crate::{
    models::collaboration::{FileChunkSpec, FILE_TRANSFER_CHUNK_BYTES},
    services::file_transfer::FILE_TRANSFER_BUFFER_BYTES,
};
use sha2::{Digest, Sha256};
use std::{future::poll_fn, time::Duration};
use tokio::time::{timeout_at, Instant};

pub(super) async fn collect<B>(
    body: B,
    spec: &FileChunkSpec,
    idle: Duration,
    total: Duration,
) -> Result<Vec<u8>, FileChunkReceiveError>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
{
    let length =
        usize::try_from(spec.byte_length).map_err(|_| FileChunkReceiveError::InvalidInput)?;
    if spec.byte_length == 0 || spec.byte_length > FILE_TRANSFER_CHUNK_BYTES {
        return Err(FileChunkReceiveError::InvalidInput);
    }
    let started = Instant::now();
    let total_deadline = started + total;
    let mut idle_deadline = started + idle;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| FileChunkReceiveError::Unavailable)?;
    let mut hash = Sha256::new();
    let mut body = Box::pin(body); // The public Body need not implement Unpin.
    loop {
        check_deadline(total_deadline, idle_deadline)?;
        let frame = timeout_at(
            total_deadline.min(idle_deadline),
            poll_fn(|cx| body.as_mut().poll_frame(cx)),
        )
        .await
        .map_err(|_| FileChunkReceiveError::TimedOut)?;
        // A caller-provided poll may itself block; timers cannot kill it, but an overdue return
        // must still fail closed before any file dispatch. Never trust is_end_stream or size_hint.
        check_deadline(total_deadline, idle_deadline)?;
        let Some(frame) = frame else {
            break;
        };
        let data = frame
            .map_err(|_| FileChunkReceiveError::InvalidResponse)?
            .into_data()
            .map_err(|_| FileChunkReceiveError::InvalidResponse)?;
        if data.len() > length - bytes.len() {
            return Err(FileChunkReceiveError::InvalidResponse);
        }
        if data.is_empty() {
            // Empty ready frames do not refresh idle time or monopolize the executor.
            tokio::task::yield_now().await;
            continue;
        }
        for part in data.chunks(FILE_TRANSFER_BUFFER_BYTES) {
            check_deadline(total_deadline, idle_deadline)?;
            bytes.extend_from_slice(part);
            hash.update(part);
            // Do not revive an expired idle window if copying/hash or thread scheduling crossed
            // its original deadline before this segment completed.
            check_deadline(total_deadline, idle_deadline)?;
            // Only actual nonempty copied bytes refresh idle time. Total time never changes.
            idle_deadline = Instant::now() + idle;
            tokio::task::yield_now().await;
        }
    }
    // Reaching expected length is insufficient: the actual message body must return None,
    // without an extra data/trailer/error frame. This is not raw TCP connection EOF.
    check_deadline(total_deadline, idle_deadline)?;
    let digest = format!("{:x}", hash.finalize());
    check_deadline(total_deadline, idle_deadline)?;
    if bytes.len() != length || digest != spec.sha256.as_str() {
        return Err(FileChunkReceiveError::InvalidResponse);
    }
    // Only the selected digest is checked; the whole-file header was declared metadata equality.
    Ok(bytes)
}

fn check_deadline(total: Instant, idle: Instant) -> Result<(), FileChunkReceiveError> {
    let now = Instant::now();
    if now >= total || now >= idle {
        Err(FileChunkReceiveError::TimedOut)
    } else {
        Ok(())
    }
}
