use crate::{local::*, script::*, *};
use axum::body::Bytes;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[tokio::test]
async fn receiver_idle_deadline_requires_eof_even_after_all_expected_data() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        for steps in [
            vec![Step::Pending],
            vec![Step::Data(data(&manifest, 0)), Step::Pending],
        ] {
            let body = Script::new(steps);
            let calls = body.calls.clone();
            tokio::time::timeout(
                Duration::from_secs(4),
                rejected(
                    &f,
                    &receiver,
                    &manifest,
                    0,
                    response(&manifest, 0, body),
                    FileChunkReceiveError::TimedOut,
                ),
            )
            .await
            .unwrap();
            assert!(calls.load(Ordering::SeqCst) > 0);
            assert!(f.tree().is_empty());
        }
        healthy(&f, &receiver, 2).await;
    })
    .await;
}

#[tokio::test]
async fn receiver_ready_empty_frames_never_refresh_idle_or_starve_the_deadline() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        let body = Script::new([Step::Data(data(&manifest, 0)), Step::EmptyForever]);
        let calls = body.calls.clone();
        tokio::time::timeout(
            Duration::from_secs(4),
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                response(&manifest, 0, body),
                FileChunkReceiveError::TimedOut,
            ),
        )
        .await
        .unwrap();
        assert!(calls.load(Ordering::SeqCst) > 2);
        assert!(f.tree().is_empty());
        healthy(&f, &receiver, 2).await;
    })
    .await;
}

#[tokio::test]
async fn receiver_total_deadline_does_not_refresh_when_real_nonempty_progress_continues() {
    checked(|f| async move {
        let receiver = FileChunkReceiver::with_limits(
            f.staging.clone(),
            1,
            Duration::from_secs(1),
            Duration::from_millis(1500),
        )
        .unwrap();
        let manifest = bytes::manifest(17, 1);
        let progress = Arc::new(AtomicUsize::new(0));
        let count = progress.clone();
        let stream = futures_util::stream::unfold(0u64, move |offset| {
            let count = count.clone();
            async move {
                if offset == 17 {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
                count.fetch_add(1, Ordering::SeqCst);
                Some((
                    Ok::<_, std::io::Error>(Bytes::from(vec![bytes::byte(offset)])),
                    offset + 1,
                ))
            }
        });
        tokio::time::timeout(
            Duration::from_secs(4),
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                response(&manifest, 0, Body::from_stream(stream)),
                FileChunkReceiveError::TimedOut,
            ),
        )
        .await
        .unwrap();
        assert!(
            progress.load(Ordering::SeqCst) >= 3,
            "nonempty progress must actually occur before total deadline"
        );
        assert!(
            progress.load(Ordering::SeqCst) < 17,
            "the body cannot reach EOF before fixed total deadline"
        );
        healthy(&f, &receiver, 2).await;
    })
    .await;
}
