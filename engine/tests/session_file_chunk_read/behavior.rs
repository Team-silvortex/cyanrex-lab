use crate::{fixture::Fixture, support::*, *};
use std::{
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};
async fn publish_share(f: &Fixture, seed: u128, length: u64) -> SessionFileChunkReadRequest {
    let actor = f.sharing.actor();
    let manifest = f.sharing.base.stage_additional(seed, length, actor).await;
    let mut attempt = f
        .sharing
        .base
        .publisher
        .prepare(
            publication_fixture::upload(seed),
            actor.principal(),
            actor.account_id(),
        )
        .await
        .unwrap();
    drop(
        auth(f)
            .source
            .publish_session_file_upload(
                &auth(f).learner_token,
                &f.sharing.base.workspace,
                grant_fixture::grant(1),
                &mut attempt,
            )
            .await
            .unwrap(),
    );
    let mut command = f.sharing.command(seed);
    command.upload_id = publication_fixture::upload(seed);
    command.expected_file = FileVersionRef::from_manifest(&manifest);
    let before = f.sharing.snapshot().await;
    let record = auth(f)
        .source
        .issue_session_file_recipient_grant(&auth(f).learner_token, &f.sharing.workspace, &command)
        .await
        .unwrap();
    let after = f.sharing.snapshot().await;
    sharing_fixture::recipient_delta(&f.sharing.schema, &before, &after, seed, false);
    assert_eq!(record.grant().file(), &command.expected_file);
    assert_eq!(record.grant().recipient(), f.sharing.recipient);
    SessionFileChunkReadRequest {
        download: SessionFileDownloadRequest {
            grant_id: command.grant_id,
            upload_id: command.upload_id,
            expected_file: command.expected_file,
        },
        chunk_index: 0,
    }
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_returns_exact_cross_chunk_bytes_and_rejects_empty_or_local_out_of_range_indices(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            healthy(f).await;
            let mut r = publish_share(f, 2, FILE_TRANSFER_CHUNK_BYTES + 17).await;
            assert_eq!(
                read(f, &r).await.unwrap(),
                expected(
                    r.download.expected_file.clone(),
                    0,
                    FILE_TRANSFER_CHUNK_BYTES
                )
            );
            r.chunk_index = 1;
            assert_eq!(
                read(f, &r).await.unwrap(),
                expected(r.download.expected_file.clone(), 1, 17)
            );
            let tails = bytes::tree(&f.sharing.base.root);
            let mut tails = tails.iter().filter(|entry| entry.name.to_string_lossy().ends_with("-1"));
            let tail = f.sharing.base.root.join(&tails.next().expect("one real tail chunk").name);
            assert!(tails.next().is_none());
            let saved = f.sharing.base.directory.join("unrequested-original-tail");
            for damage in ["corrupt", "missing"] {
                std::fs::rename(&tail, &saved).unwrap();
                let original = bytes::snapshot(&saved);
                if damage == "corrupt" {
                    let mut wrong = expected(r.download.expected_file.clone(), 1, 17).bytes;
                    wrong[0] ^= 1;
                    let mut file = std::fs::OpenOptions::new().write(true).create_new(true)
                        .mode(0o600).open(&tail).unwrap();
                    file.write_all(&wrong).unwrap();
                    file.set_permissions(std::fs::Permissions::from_mode(0o400)).unwrap();
                    drop(file);
                }
                r.chunk_index = 0;
                assert_eq!(read(f, &r).await.unwrap(), expected(r.download.expected_file.clone(), 0, FILE_TRANSFER_CHUNK_BYTES),
                    "only the selected chunk is observed; an unrequested {damage} tail is not a fresh whole-file check");
                r.chunk_index = 1;
                assert_eq!(read(f, &r).await, Err(SessionFileChunkReadError::Unavailable),
                    "requesting the actual {damage} tail must disclose no bytes");
                assert_eq!(bytes::snapshot(&saved), original);
                if damage == "corrupt" { std::fs::remove_file(&tail).unwrap(); }
                std::fs::rename(&saved, &tail).unwrap();
                assert_eq!(read(f, &r).await.unwrap(), expected(r.download.expected_file.clone(), 1, 17));
            }
            r.chunk_index = 2;
            assert_eq!(
                read(f, &r).await,
                Err(SessionFileChunkReadError::InvalidInput)
            );
            r = f.request();
            r.chunk_index = 1;
            assert_eq!(
                read(f, &r).await,
                Err(SessionFileChunkReadError::InvalidInput)
            );
            let empty = publish_share(f, 3, 0).await;
            assert_eq!(
                read(f, &empty).await,
                Err(SessionFileChunkReadError::InvalidInput),
                "an empty file has no invented chunk-zero byte response"
            );
            let before = f.sharing.snapshot().await;
            let retired = f
                .sharing
                .base
                .grants
                .store
                .revoke(
                    grant_fixture::grant(1),
                    f.sharing.base.grants.issuer(),
                    sharing_fixture::revision(1),
                )
                .await
                .unwrap();
            let after = f.sharing.snapshot().await;
            assert_eq!(retired.grant().state(), FileGrantState::Revoked);
            sharing_fixture::publisher_revocation_delta(&before, &after);
            healthy(f).await;
            // A byte result is an inert verified read-time snapshot, not a publisher grant or
            // acknowledgement-time pathname seal, and does not continuously watch later revocations.
            let value = auth(f)
                .source
                .read_session_file_chunk(&auth(f).target_token, &f.workspace, &f.request())
                .await
                .unwrap();
            let debug = format!("{value:?}");
            assert!(debug.contains("<redacted>"));
            assert!(!debug.contains("bytes: ["));
            drop(value);
            drain(f).await;
        })
    })
    .await;
}
