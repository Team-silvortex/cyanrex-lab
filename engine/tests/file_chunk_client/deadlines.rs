use crate::{fixture::*, support::*, *};
use axum::body::Bytes;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn client_header_deadline_and_later_body_timeout_have_distinct_lifetimes() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 0);
            reply.header_gate = Some(gate.clone());
            f.set_reply(reply);
            let mut c = config(1);
            c.idle_timeout = Duration::from_secs(2);
            c.total_timeout = Duration::from_secs(4);
            let client = FileChunkClient::with_config(&f.origin, f.staging.clone(), c).unwrap();
            tokio::time::timeout(
                Duration::from_secs(4),
                denied(f, &client, &f.manifest, 0, FileChunkClientError::TimedOut),
            )
            .await
            .unwrap();
            assert_eq!(
                f.observed().len(),
                1,
                "the header wait must be a real accepted request"
            );
            gate.add_permits(1);
            let manifest = bytes::manifest(17, 2);
            let mut reply = Reply::good(&manifest, 0);
            let data = reply.pieces[0].data.clone();
            reply.pieces = vec![
                Piece {
                    delay: Duration::ZERO,
                    data: data.slice(..8),
                    gate: None,
                },
                Piece {
                    delay: Duration::from_millis(1200),
                    data: data.slice(8..),
                    gate: None,
                },
            ];
            f.set_reply(reply);
            client
                .fetch_and_stage(TOKEN, grant(), upload(), &manifest, 0)
                .await
                .expect("body may take longer than header_timeout after headers arrived");
            saved(&f.root.join(name(&manifest)), &manifest)
                .unwrap()
                .verified
                .unwrap();
            healthy(f, &client, 3).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_body_idle_timeout_discards_partial_bytes_and_restores_capacity() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 0);
            let data = reply.pieces[0].data.clone();
            reply.pieces = vec![
                Piece {
                    delay: Duration::ZERO,
                    data: data.slice(..8),
                    gate: None,
                },
                Piece {
                    delay: Duration::ZERO,
                    data: data.slice(8..),
                    gate: Some(gate.clone()),
                },
            ];
            f.set_reply(reply);
            let client = client(f, 1);
            tokio::time::timeout(
                Duration::from_secs(4),
                denied(f, &client, &f.manifest, 0, timed_body()),
            )
            .await
            .unwrap();
            assert_eq!(f.observed().len(), 1);
            assert!(bytes::tree(&f.root).is_empty());
            gate.add_permits(1);
            healthy(f, &client, 2).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_fixed_body_total_timeout_survives_continuing_nonempty_network_progress() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let mut c = config(1);
            c.total_timeout = Duration::from_millis(1500);
            let client = FileChunkClient::with_config(&f.origin, f.staging.clone(), c).unwrap();
            let mut reply = Reply::good(&f.manifest, 0);
            reply.pieces = (0..17)
                .map(|i| Piece {
                    delay: if i == 0 {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(200)
                    },
                    data: Bytes::from(vec![bytes::byte(i)]),
                    gate: None,
                })
                .collect();
            f.set_reply(reply);
            tokio::time::timeout(
                Duration::from_secs(4),
                denied(f, &client, &f.manifest, 0, timed_body()),
            )
            .await
            .unwrap();
            assert_eq!(f.observed().len(), 1);
            assert!(
                f.produced() >= 3 && f.produced() < 17,
                "real nonempty server progress occurs but cannot finish within total deadline"
            );
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client, 2).await;
        })
    })
    .await;
}
