//! One explicit caller request, never a file-job stop or a channel closure side effect.
use tokio::sync::oneshot;

/// Move-only request handle. Dropping it without calling cancel leaves delivery uncancelled.
pub struct FileDeliveryCancelHandle {
    sender: oneshot::Sender<()>,
}

/// Move-only token consumed by one controlled delivery, with no public raw channel or polling API.
pub struct FileDeliveryCancellation {
    receiver: oneshot::Receiver<()>,
}

impl FileDeliveryCancelHandle {
    pub fn channel() -> (Self, FileDeliveryCancellation) {
        let (sender, receiver) = oneshot::channel();
        (Self { sender }, FileDeliveryCancellation { receiver })
    }

    /// Request stopping this call's waiting when it is next polled; late closure is harmless.
    /// This neither kills OS work nor authorizes retry, overwrite or cleanup.
    pub fn cancel(self) {
        let _ = self.sender.send(());
    }
}

impl FileDeliveryCancellation {
    pub(super) async fn requested(self) {
        if self.receiver.await.is_err() {
            // Handle drop is not a cancellation request, including before the first call poll.
            std::future::pending::<()>().await;
        }
    }
}
