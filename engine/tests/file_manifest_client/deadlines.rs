use crate::{fixture::*, support::*, *};
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn manifest_header_deadline_stops_accepted_request_but_does_not_continue_into_valid_body() {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let mut limits = config(1);
            limits.idle_timeout = Duration::from_secs(2);
            limits.total_timeout = Duration::from_secs(4);
            let client = f.client_with(limits);
            denied(f, &client, FileManifestClientError::TimedOut).await;
            assert_eq!(
                f.request_count(),
                1,
                "the header timeout follows an actual accepted GET"
            );
            gate.add_permits(1);
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            let data = reply.pieces[0].data.clone();
            reply.pieces = vec![
                Piece::now(data.slice(..8)),
                Piece::delayed(data.slice(8..), Duration::from_millis(1200)),
            ];
            f.set_reply(Kind::Manifest, reply);
            assert_eq!(
                metadata(f, &client).await.unwrap(),
                f.manifest,
                "a valid body can finish after the separate one-second header deadline"
            );
            assert_eq!(f.request_count(), 2);
            assert!(bytes::tree(&f.root).is_empty());
            for request in f.take_requests() {
                assert_request(&request, &f.manifest, None);
            }
        })
    })
    .await;
}

#[tokio::test]
async fn manifest_idle_deadline_rejects_actual_partial_body_and_restores_metadata_capacity() {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            let data = reply.pieces[0].data.clone();
            reply.pieces = vec![
                Piece::now(data.slice(..8)),
                Piece::held(data.slice(8..), gate.clone()),
            ];
            f.set_reply(Kind::Manifest, reply);
            let client = f.client_with(config(1));
            denied(f, &client, FileManifestClientError::TimedOut).await;
            assert_eq!(f.request_count(), 1);
            assert_eq!(
                f.produced(Kind::Manifest),
                1,
                "the real prefix is emitted while its tail remains held"
            );
            assert!(bytes::tree(&f.root).is_empty());
            gate.add_permits(1);
            healthy(f, &client).await;
            assert_eq!(f.request_count(), 2);
        })
    })
    .await;
}

#[tokio::test]
async fn manifest_fixed_total_deadline_does_not_refresh_with_continuing_nonempty_body_progress() {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let mut limits = config(1);
            limits.total_timeout = Duration::from_millis(1500);
            let client = f.client_with(limits);
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            let data = reply.pieces[0].data.clone();
            reply.pieces = (0..17)
                .map(|index| {
                    Piece::delayed(
                        data.slice(data.len() * index / 17..data.len() * (index + 1) / 17),
                        if index == 0 {
                            Duration::ZERO
                        } else {
                            Duration::from_millis(200)
                        },
                    )
                })
                .collect();
            f.set_reply(Kind::Manifest, reply);
            denied(f, &client, FileManifestClientError::TimedOut).await;
            assert_eq!(f.request_count(), 1);
            assert!(
                f.produced(Kind::Manifest) >= 3 && f.produced(Kind::Manifest) < 17,
                "actual nonempty network progress cannot extend the fixed body total deadline"
            );
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client).await;
            assert_eq!(f.request_count(), 2);
        })
    })
    .await;
}
