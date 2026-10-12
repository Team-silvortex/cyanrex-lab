use crate::{local::*, script::*, *};
use std::{future::Future, pin::Pin, sync::atomic::Ordering, task::Poll};

async fn pending_once<F: Future>(future: &mut Pin<Box<F>>) {
    futures_util::future::poll_fn(|cx| {
        assert!(
            future.as_mut().poll(cx).is_pending(),
            "the real body must be parked before measuring admission"
        );
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn receiver_default_four_slots_and_clones_share_while_new_receivers_are_independent() {
    checked(|f| async move {
        let receiver = FileChunkReceiver::new(f.staging.clone());
        let manifest = bytes::manifest(17, 1);
        let mut held = Vec::new();
        for _ in 0..4 {
            let r = receiver.clone();
            let m = manifest.clone();
            let body = Script::unread();
            let calls = body.calls.clone();
            let mut pending =
                Box::pin(async move { r.receive_and_stage(&m, 0, response(&m, 0, body)).await });
            pending_once(&mut pending).await;
            assert!(calls.load(Ordering::SeqCst) > 0);
            held.push(pending);
        }
        let body = Script::unread();
        let calls = body.calls.clone();
        rejected(
            &f,
            &receiver.clone(),
            &manifest,
            0,
            response(&manifest, 0, body),
            FileChunkReceiveError::CapacityExhausted,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let body = Script::unread();
        let calls = body.calls.clone();
        let mut invalid = response(&manifest, 0, body);
        invalid.headers_mut().remove("content-length");
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            invalid,
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let body = Script::unread();
        let calls = body.calls.clone();
        rejected(
            &f,
            &receiver,
            &manifest,
            1,
            response(&manifest, 0, body),
            FileChunkReceiveError::InvalidInput,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        // A newly constructed receiver has an independent receive budget, even with the same staging.
        let independent = f.receiver(1);
        healthy(&f, &independent, 2).await;
        assert_eq!(
            f.tree().len(),
            1,
            "pending receives must not open output entries"
        );
        drop(held.pop());
        healthy(&f, &receiver, 3).await;
        drop(held);
        healthy(&f, &receiver.clone(), 4).await;
    })
    .await;
}

#[tokio::test]
async fn receiver_cancelled_partial_body_releases_receive_capacity_without_staging() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        let body = Script::new([Step::Data(data(&manifest, 0).slice(..8)), Step::Pending]);
        let calls = body.calls.clone();
        let mut pending =
            Box::pin(receiver.receive_and_stage(&manifest, 0, response(&manifest, 0, body)));
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            futures_util::future::poll_fn(|cx| {
                assert!(pending.as_mut().poll(cx).is_pending());
                if calls.load(Ordering::SeqCst) >= 2 {
                    Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }),
        )
        .await
        .unwrap();
        assert!(calls.load(Ordering::SeqCst) >= 2);
        assert!(
            f.tree().is_empty(),
            "partial network bytes may not become staging files"
        );
        let body = Script::unread();
        let calls = body.calls.clone();
        rejected(
            &f,
            &receiver.clone(),
            &manifest,
            0,
            response(&manifest, 0, body),
            FileChunkReceiveError::CapacityExhausted,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(pending);
        assert!(f.tree().is_empty());
        healthy(&f, &receiver, 2).await;
    })
    .await;
}
