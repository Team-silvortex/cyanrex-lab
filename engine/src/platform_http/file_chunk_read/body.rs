use crate::services::auth_service::durable_source::SessionFileChunkRead;
use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn first_frame_deadline() -> Instant {
    Instant::now() + FIRST_FRAME_TIMEOUT
}

struct ByteOwner {
    chunk: SessionFileChunkRead,
    // Drop the immutable bytes/proof before making the independent HTTP slot available again.
    _http: OwnedSemaphorePermit,
}

impl AsRef<[u8]> for ByteOwner {
    fn as_ref(&self) -> &[u8] {
        self.chunk.bytes()
    }
}

pub(super) fn response(
    chunk: SessionFileChunkRead,
    permit: OwnedSemaphorePermit,
    deadline: Instant,
) -> Body {
    // from_owner neither rehashes nor copies payload bytes. Both leases follow this allocation
    // through all Data Bytes clones/slices, including after EOF or the response body is dropped.
    let data = Bytes::from_owner(ByteOwner {
        chunk,
        _http: permit,
    });
    Body::new(ChunkBody {
        data: Some(data),
        deadline,
    })
}

struct ChunkBody {
    data: Option<Bytes>,
    deadline: Instant,
}

impl HttpBody for ChunkBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let Some(data) = self.data.take() else {
            return Poll::Ready(None);
        };
        if Instant::now() >= self.deadline {
            // One static error, no prefix/data frame. An unpolled body has no detached timer:
            // it retains its owner until first poll or drop. This is not a delivery deadline.
            drop(data);
            return Poll::Ready(Some(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "binary first frame deadline exceeded",
            ))));
        }
        Poll::Ready(Some(Ok(Frame::data(data))))
    }

    fn is_end_stream(&self) -> bool {
        self.data.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.data.as_ref().map_or(0, |data| data.len() as u64))
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
