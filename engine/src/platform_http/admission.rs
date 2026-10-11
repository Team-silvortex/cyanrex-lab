//! Fail-fast slots covering admitted requests and their unsent Axum response bodies.
//! No queue, detached task, command timeout, rollback claim or underlying worker accounting.
use super::errors::HttpError;
use axum::{
    body::{Body, Bytes, HttpBody},
    http::StatusCode,
    response::Response,
};
use http_body::{Frame, SizeHint};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) fn acquire(semaphore: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit, HttpError> {
    semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| HttpError::new(StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"))
}

pub(super) fn response(mut response: Response, permit: OwnedSemaphorePermit) -> Response {
    let inner = std::mem::replace(response.body_mut(), Body::empty());
    *response.body_mut() = Body::new(AdmittedBody {
        inner,
        permit: Some(permit),
    });
    response
}

struct AdmittedBody {
    inner: Body,
    // Dropping a response or cancelled request releases its slot, not already-dispatched work.
    permit: Option<OwnedSemaphorePermit>,
}

impl HttpBody for AdmittedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = Pin::new(&mut self.inner).poll_frame(cx);
        // Consumers can stop after the last frame if is_end_stream is true, without polling None.
        // Transfer of that frame is body completion, not proof of delivery to the remote client.
        if matches!(&frame, Poll::Ready(None | Some(Err(_)))) || self.inner.is_end_stream() {
            self.permit.take();
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn response_body_error_releases_capacity_without_dropping_the_body() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = acquire(&semaphore).unwrap_or_else(|_| panic!("fresh slot"));
        let broken = Body::from_stream(futures_util::stream::iter([Err::<Bytes, _>(
            std::io::Error::other("synthetic outgoing body error"),
        )]));
        let mut body = response(Response::new(broken), permit).into_body();
        assert_eq!(semaphore.available_permits(), 0);
        assert!(body.frame().await.unwrap().is_err());
        assert_eq!(semaphore.available_permits(), 1);
        // Keep the failed body allocation alive while another slot is admitted.
        assert!(acquire(&semaphore).is_ok());
        drop(body);
    }
}
