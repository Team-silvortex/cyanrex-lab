use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc};

#[tokio::test]
async fn empty_download_creates_actual_empty_output_without_http_and_rejects_drift_and_bad_token() {
    checked(Fixture::new(0).await, |f| {
        Box::pin(async move {
            let client = f.client(1);
            assert_eq!(
                client
                    .fetch_and_assemble("bad-token", grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::InvalidInput)
            );
            assert!(bytes::tree(&f.root).is_empty());
            let assembled = client
                .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                .await
                .unwrap();
            assert_eq!(
                assembled.file(),
                &FileVersionRef::from_manifest(&f.manifest)
            );
            let observed =
                observe_output(&f.root.join(output_name(&f.manifest)), &f.manifest).unwrap();
            assert_eq!((observed.length, observed.observed_length), (0, 0));
            assert_eq!(observed.mode & 0o7777, 0o400);
            assert_eq!(observed.links, 1);
            assert!(observed.same_visible_inode && observed.pattern_matches);
            observed.verified.unwrap();
            assert_eq!(observed.digest, f.manifest.sha256().to_string());
            let before = bytes::tree(&f.root);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::Assembly(ChunkStagingError::Conflict))
            );
            assert_eq!(bytes::tree(&f.root), before);
            fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
            let drifted = bytes::snapshot(&f.root);
            assert_eq!(
                client
                    .fetch_and_assemble("bad-token", grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::InvalidInput)
            );
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::Assembly(ChunkStagingError::Unavailable))
            );
            assert_eq!(bytes::snapshot(&f.root), drifted);
            assert_eq!(bytes::tree(&f.root), before);
            assert_eq!(
                f.request_count(),
                0,
                "an empty manifest never invents a chunk request"
            );
        })
    })
    .await;
}

#[tokio::test]
async fn default_four_flows_share_between_clones_reject_inputs_first_and_independent_clients_add_capacity(
) {
    checked(Fixture::new(17).await, |f| {
        Box::pin(async move {
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, 0);
            reply.header_gate = Some(gate.clone());
            f.set_reply(0, reply);
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let mut held = Vec::new();
            for count in 1..=4 {
                let client = client.clone();
                let manifest = f.manifest.clone();
                let mut pending = Box::pin(async move {
                    client
                        .fetch_and_assemble(TOKEN, grant(), upload(), &manifest)
                        .await
                });
                drive_to_request(f, count, &mut pending).await;
                held.push(pending);
            }
            assert_eq!(
                client
                    .clone()
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(FileDownloadError::CapacityExhausted)
            );
            for token in [
                "bad-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
            ] {
                assert_eq!(
                    client
                        .fetch_and_assemble(token, grant(), upload(), &f.manifest)
                        .await
                        .err(),
                    Some(FileDownloadError::InvalidInput)
                );
            }
            let mut other = scope();
            other.workspace_id = Uuid::from_u128(98).try_into().unwrap();
            let wrong_scope = changed(
                &f.manifest,
                "workspace",
                serde_json::to_value(other).unwrap(),
            );
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &wrong_scope)
                    .await
                    .err(),
                Some(FileDownloadError::InvalidInput)
            );
            assert_eq!(f.request_count(), 4);
            assert!(bytes::tree(&f.root).is_empty());
            f.clear_reply(0);
            let independent = f.client(1);
            let assembled = independent
                .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                .await
                .unwrap();
            assert_eq!(
                assembled.file(),
                &FileVersionRef::from_manifest(&f.manifest)
            );
            assert_eq!(
                f.request_count(),
                5,
                "independent construction has its own flow and request budgets"
            );
            let before = bytes::tree(&f.root);
            drop(held);
            gate.add_permits(4);
            assert_eq!(
                client
                    .fetch_and_assemble(TOKEN, grant(), upload(), &f.manifest)
                    .await
                    .err(),
                Some(staging_error(0, ChunkStagingError::Conflict))
            );
            assert_eq!(
                f.request_count(),
                6,
                "cancelled header flows release their shared slots"
            );
            assert_eq!(bytes::tree(&f.root), before);
        })
    })
    .await;
}
