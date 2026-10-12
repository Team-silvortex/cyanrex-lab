use crate::{local::*, script::*, *};
use std::{sync::atomic::Ordering, time::Duration};

#[tokio::test]
async fn receiver_configuration_bounds_and_cheap_manifest_index_rejection_do_not_poll_body() {
    checked(|f| async move {
        for (capacity, idle, total) in [
            (0, 1_000, 1_000),
            (17, 1_000, 1_000),
            (1, 999, 1_000),
            (1, 60_001, 60_001),
            (1, 1_000, 999),
            (1, 1_000, 300_001),
        ] {
            assert_eq!(
                FileChunkReceiver::with_limits(
                    f.staging.clone(),
                    capacity,
                    Duration::from_millis(idle),
                    Duration::from_millis(total)
                )
                .err(),
                Some(FileChunkReceiveError::InvalidInput)
            );
        }
        for (capacity, idle, total) in [(1, 1_000, 1_000), (16, 60_000, 300_000), (4, 1_250, 1_500)]
        {
            assert!(FileChunkReceiver::with_limits(
                f.staging.clone(),
                capacity,
                Duration::from_millis(idle),
                Duration::from_millis(total)
            )
            .is_ok());
        }
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        let mut value = serde_json::to_value(&manifest).unwrap();
        value["workspace"]["workspace_id"] = serde_json::json!(Uuid::new_v4());
        let foreign: FileTransferManifest = serde_json::from_value(value).unwrap();
        let mut value = serde_json::to_value(&manifest).unwrap();
        value["workspace"]["authority_id"] = serde_json::json!(Uuid::new_v4());
        let foreign_authority: FileTransferManifest = serde_json::from_value(value).unwrap();
        for (m, index) in [
            (&foreign, 0),
            (&foreign_authority, 0),
            (&manifest, 1),
            (&manifest, MAX_FILE_TRANSFER_CHUNKS as u32),
        ] {
            let body = Script::unread();
            let calls = body.calls.clone();
            rejected(
                &f,
                &receiver,
                m,
                index,
                response(&manifest, 0, body),
                FileChunkReceiveError::InvalidInput,
            )
            .await;
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        let empty = bytes::manifest(0, 2);
        let body = Script::unread();
        let calls = body.calls.clone();
        rejected(
            &f,
            &receiver,
            &empty,
            0,
            response(&manifest, 0, body),
            FileChunkReceiveError::InvalidInput,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        healthy(&f, &receiver, 3).await;
    })
    .await;
}
