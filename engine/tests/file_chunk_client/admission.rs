use crate::{fixture::*, support::*, *};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

async fn drive_to_request<F: Future>(f: &Fixture, count: usize, pending: &mut Pin<Box<F>>) {
    tokio::select! {
        _=pending.as_mut()=>panic!("request must remain in the real held header/body phase"),
        _=f.wait_requests(count)=>{}
    }
}

#[tokio::test]
async fn client_default_four_requests_clones_share_and_independent_clients_add_capacity() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 0);
            reply.header_gate = Some(gate.clone());
            f.set_reply(reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut held = Vec::new();
            for count in 1..=4 {
                let client = client.clone();
                let manifest = f.manifest.clone();
                let mut pending = Box::pin(async move {
                    client
                        .fetch_and_stage(TOKEN, grant(), upload(), &manifest, 0)
                        .await
                });
                drive_to_request(f, count, &mut pending).await;
                held.push(pending);
            }
            denied(
                f,
                &client.clone(),
                &f.manifest,
                0,
                FileChunkClientError::CapacityExhausted,
            )
            .await;
            assert_eq!(f.observed().len(), 4);
            assert_eq!(
                client
                    .fetch_and_stage("bad-token", grant(), upload(), &f.manifest, 0)
                    .await
                    .err(),
                Some(FileChunkClientError::InvalidInput)
            );
            assert_eq!(
                client
                    .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 1)
                    .await
                    .err(),
                Some(FileChunkClientError::InvalidInput)
            );
            assert_eq!(f.observed().len(), 4);
            // Independent construction does not consume any of the original client's four slots.
            let independent = crate::support::client(f, 1);
            healthy(f, &independent, 2).await;
            assert_eq!(f.observed().len(), 5);
            drop(held.pop());
            healthy(f, &client, 3).await;
            assert_eq!(f.observed().len(), 6);
            drop(held);
            gate.add_permits(4);
            healthy(f, &client.clone(), 4).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_cancelled_header_request_releases_gate_and_does_not_create_output() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 0);
            reply.header_gate = Some(gate.clone());
            f.set_reply(reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut pending =
                Box::pin(client.fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0));
            drive_to_request(f, 1, &mut pending).await;
            assert!(bytes::tree(&f.root).is_empty());
            drop(pending);
            gate.add_permits(1);
            healthy(f, &client, 2).await;
            assert_eq!(f.observed().len(), 2);
        })
    })
    .await;
}

#[tokio::test]
async fn client_cancelled_partial_body_does_not_save_and_shared_slot_recovers() {
    checked(Fixture::new().await, |f| Box::pin(async move {
        let gate=Arc::new(tokio::sync::Semaphore::new(0));let mut reply=Reply::good(&f.manifest,0);let data=reply.pieces[0].data.clone();
        reply.pieces=vec![Piece{delay:Duration::ZERO,data:data.slice(..8),gate:None},Piece{delay:Duration::ZERO,data:data.slice(8..),gate:Some(gate.clone())}];f.set_reply(reply);
        let client=crate::support::client(f,1);let mut pending=Box::pin(client.fetch_and_stage(TOKEN,grant(),upload(),&f.manifest,0));drive_to_request(f,1,&mut pending).await;
        // Real transport has yielded the first data piece, while the selected tail is held.
        // This is a pre-staging cancellation, not evidence of an actual dispatched disk job.
        tokio::select!{_=&mut pending=>panic!("held partial body must not complete"),_=f.wait_pieces(1)=>{}}
        denied(f,&client.clone(),&f.manifest,0,FileChunkClientError::CapacityExhausted).await;assert_eq!(f.observed().len(),1);
        assert!(bytes::tree(&f.root).is_empty());drop(pending);gate.add_permits(1);healthy(f,&client,2).await;
    })).await;
}
