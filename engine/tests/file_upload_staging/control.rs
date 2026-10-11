use super::bytes::Pattern;
use std::{
    io::{self, Read},
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};
use tokio::sync::oneshot;

pub struct Release(Option<mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(release) = self.0.take() {
            let _ = release.send(());
        }
    }
}

pub struct HeldReader {
    pattern: Pattern,
    block_after_calls: usize,
    entered: Option<oneshot::Sender<()>>,
    wait: mpsc::Receiver<()>,
    dropped: Option<oneshot::Sender<()>>,
}
impl Read for HeldReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.pattern.calls.load(Ordering::SeqCst) >= self.block_after_calls {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
                self.wait
                    .recv_timeout(Duration::from_secs(10))
                    .expect("held worker was not released");
            }
        }
        self.pattern.read(bytes)
    }
}
impl Drop for HeldReader {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

pub fn held(
    start: u64,
    length: u64,
    after_calls: usize,
) -> (
    HeldReader,
    Release,
    oneshot::Receiver<()>,
    oneshot::Receiver<()>,
) {
    let (release, wait) = mpsc::channel();
    let (entered, started) = oneshot::channel();
    let (dropped, finished) = oneshot::channel();
    (
        HeldReader {
            pattern: Pattern::new(start, length),
            block_after_calls: after_calls,
            entered: Some(entered),
            wait,
            dropped: Some(dropped),
        },
        Release(Some(release)),
        started,
        finished,
    )
}
pub async fn signal(receiver: oneshot::Receiver<()>) {
    tokio::time::timeout(Duration::from_secs(2), receiver)
        .await
        .expect("worker did not reach an actual barrier")
        .unwrap();
}
