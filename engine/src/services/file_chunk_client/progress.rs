//! Bounded local observation only: no byte authority, publication, receipt, recovery or callback.
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDeliveryPhase {
    FetchingManifest,
    StagingChunks,
    Assembling,
    Saving,
    Complete,
}

/// Latest caller-observed prefix, not current file health or a transferable acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDeliverySnapshot {
    phase: FileDeliveryPhase,
    staged_chunks: u32,
    staged_bytes: u64,
    total_bytes: u64,
    total_chunks: Option<u32>,
}

impl FileDeliverySnapshot {
    pub fn phase(&self) -> FileDeliveryPhase {
        self.phase
    }
    pub fn staged_chunks(&self) -> u32 {
        self.staged_chunks
    }
    pub fn staged_bytes(&self) -> u64 {
        self.staged_bytes
    }
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    pub fn total_chunks(&self) -> Option<u32> {
        self.total_chunks
    }
}

/// Move-only producer consumed by one explicit delivery. No sender or injected callback is public.
pub struct FileDeliveryObserver {
    sender: watch::Sender<Option<FileDeliverySnapshot>>,
}

/// One overwriteable snapshot slot. Slow readers may skip phases; no retained event history exists.
pub struct FileDeliveryProgress {
    receiver: watch::Receiver<Option<FileDeliverySnapshot>>,
}

impl FileDeliveryObserver {
    pub fn channel() -> (Self, FileDeliveryProgress) {
        let (sender, receiver) = watch::channel(None);
        (Self { sender }, FileDeliveryProgress { receiver })
    }

    pub(super) fn report(
        &self,
        phase: FileDeliveryPhase,
        staged_chunks: u32,
        staged_bytes: u64,
        total_bytes: u64,
        total_chunks: Option<u32>,
    ) {
        // Observation is optional. Losing the receiver never becomes a transfer error or await.
        let _ = self.sender.send_replace(Some(FileDeliverySnapshot {
            phase,
            staged_chunks,
            staged_bytes,
            total_bytes,
            total_chunks,
        }));
    }
}

impl FileDeliveryProgress {
    /// Copies metadata and releases the internal watch borrow before returning.
    pub fn latest(&self) -> Option<FileDeliverySnapshot> {
        *self.receiver.borrow()
    }

    /// None means observation ended, never that pending file work stopped or rolled back.
    pub async fn changed(&mut self) -> Option<FileDeliverySnapshot> {
        self.receiver.changed().await.ok()?;
        *self.receiver.borrow_and_update()
    }
}

#[cfg(test)]
#[path = "progress_file_tests.rs"]
mod file_tests;
#[cfg(test)]
#[path = "progress_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "progress_watch_tests.rs"]
mod watch_tests;
#[cfg(test)]
#[path = "progress_work_tests.rs"]
mod work_tests;
