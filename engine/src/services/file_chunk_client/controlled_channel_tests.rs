use super::{test_support::FileDeliveryCancelHandle, *};
use std::{
    future::{poll_fn, ready, Future},
    task::Poll,
};

#[tokio::test]
async fn channel_drop_is_not_a_request_and_explicit_request_wins_the_same_poll_ready_result() {
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    drop(handle);
    let mut waiting = Box::pin(cancellation.requested());
    assert!(
        poll_fn(|cx| match waiting.as_mut().poll(cx) {
            Poll::Pending => Poll::Ready(true),
            Poll::Ready(_) => Poll::Ready(false),
        })
        .await
    );
    drop(waiting);

    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    drop(handle);
    assert_eq!(
        wait_for_delivery(cancellation, ready(Ok::<_, FileGrantedSaveError>(7))).await,
        Ok(7)
    );
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    drop(handle);
    assert_eq!(
        wait_for_delivery(
            cancellation,
            ready(Err::<(), _>(FileGrantedSaveError::CapacityExhausted))
        )
        .await,
        Err(FileControlledSaveError::Delivery(
            FileGrantedSaveError::CapacityExhausted
        ))
    );

    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    handle.cancel();
    assert_eq!(
        wait_for_delivery(cancellation, ready(Ok::<_, FileGrantedSaveError>(7))).await,
        Err(FileControlledSaveError::Cancelled)
    );
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    handle.cancel();
    assert_eq!(
        wait_for_delivery(
            cancellation,
            ready(Err::<(), _>(FileGrantedSaveError::CapacityExhausted))
        )
        .await,
        Err(FileControlledSaveError::Cancelled)
    );

    // Dropping an unused/finished token is not a late request failure or global cancellation.
    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
    drop(cancellation);
    handle.cancel();
    // This is exact helper/channel readiness evidence, not filesystem ACK or immediate preemption.
}
