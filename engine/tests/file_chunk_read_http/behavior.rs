use crate::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_cross_chunk_selected_only_binary_is_exact_and_private() {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let route = app(f);
            healthy(f, &route).await;
            let actor = f.sharing.actor();
            let manifest = f
                .sharing
                .base
                .stage_additional(2, FILE_TRANSFER_CHUNK_BYTES + 17, actor)
                .await;
            let mut attempt = f
                .sharing
                .base
                .publisher
                .prepare(
                    publication_fixture::upload(2),
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
            let mut command = f.sharing.command(2);
            command.upload_id = publication_fixture::upload(2);
            command.expected_file = FileVersionRef::from_manifest(&manifest);
            auth(f)
                .source
                .issue_session_file_recipient_grant(
                    &auth(f).learner_token,
                    &f.sharing.workspace,
                    &command,
                )
                .await
                .unwrap();
            let mut r = SessionFileChunkReadRequest {
                download: SessionFileDownloadRequest {
                    grant_id: command.grant_id,
                    upload_id: command.upload_id,
                    expected_file: command.expected_file,
                },
                chunk_index: 0,
            };
            assert_eq!(
                binary(f, &route, &r).await,
                (0..FILE_TRANSFER_CHUNK_BYTES)
                    .map(bytes::byte)
                    .collect::<Vec<_>>()
            );
            r.chunk_index = 1;
            assert_eq!(
                binary(f, &route, &r).await,
                (FILE_TRANSFER_CHUNK_BYTES..FILE_TRANSFER_CHUNK_BYTES + 17)
                    .map(bytes::byte)
                    .collect::<Vec<_>>()
            );
            let mut tails = bytes::tree(&f.sharing.base.root)
                .into_iter()
                .filter(|e| e.name.to_string_lossy().ends_with("-1"));
            let tail = f.sharing.base.root.join(tails.next().unwrap().name);
            assert!(tails.next().is_none());
            let saved = f.sharing.base.directory.join("unrequested-tail-http");
            std::fs::rename(&tail, &saved).unwrap();
            let saved_before = bytes::snapshot(&saved);
            r.chunk_index = 0;
            assert_eq!(
                binary(f, &route, &r).await,
                (0..FILE_TRANSFER_CHUNK_BYTES)
                    .map(bytes::byte)
                    .collect::<Vec<_>>(),
                "unrequested missing tail is not a wholefile observation"
            );
            r.chunk_index = 1;
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&r)),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
            assert_eq!(bytes::snapshot(&saved), saved_before);
            std::fs::rename(&saved, &tail).unwrap();
            assert_eq!(
                binary(f, &route, &r).await,
                (FILE_TRANSFER_CHUNK_BYTES..FILE_TRANSFER_CHUNK_BYTES + 17)
                    .map(bytes::byte)
                    .collect::<Vec<_>>()
            );
            r.chunk_index = 2;
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&r)),
                StatusCode::BAD_REQUEST,
                "invalid_request",
            )
            .await;
            let before = f.sharing.snapshot().await;
            f.sharing
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
            sharing_fixture::publisher_revocation_delta(&before, &after);
            healthy(f, &route).await;
            drain(f).await;
        })
    })
    .await;
}
