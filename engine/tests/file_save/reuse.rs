use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn held_original_ack_owns_no_workers_unpolled_save_is_inert_and_explicit_different_name_copy_is_not_resume(
) {
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let source = bytes::tree(&f.source_root);
            let future = f
                .store
                .save_assembled_file(&f.manifest, &assembled, &f.target);
            drop(future);
            assert!(bytes::tree(&f.target_root).is_empty());
            assert_eq!(bytes::tree(&f.source_root), source);
            let additional = bytes::manifest(17, 2);
            let spec = additional.chunk(0).unwrap();
            within(f.store.stage_chunk(
                &additional,
                0,
                bytes::Pattern::new(spec.offset, spec.byte_length),
            ))
            .await
            .unwrap();
            within(f.store.verify_staged_chunk(&additional, 0))
                .await
                .unwrap();
            within(f.store.verify_staged_chunk(&f.manifest, 0))
                .await
                .unwrap();
            let other_ack = within(f.store.assemble_staged_file(&additional))
                .await
                .unwrap();
            assert_eq!(
                other_ack.file(),
                &FileVersionRef::from_manifest(&additional)
            );
            let before = bytes::tree(&f.source_root);
            assert_eq!(
                save(f, &assembled, &f.target).await.unwrap(),
                FileVersionRef::from_manifest(&f.manifest)
            );
            let first = bytes::snapshot(&f.target_path());
            denied(f, &assembled, FileSaveError::Conflict).await;
            let second_leaf = "explicit-second-copy.bin";
            let second_target = f.target_for(second_leaf);
            assert_eq!(
                save(f, &assembled, &second_target).await.unwrap(),
                FileVersionRef::from_manifest(&f.manifest)
            );
            let first_identity = assert_output(f, &f.target_path(), &f.manifest);
            let second_identity = assert_output(f, &f.target_root.join(second_leaf), &f.manifest);
            let original = observe(&f.source_path(), &f.manifest).unwrap().identity;
            assert_ne!(first_identity, second_identity);
            assert_ne!(first_identity, original);
            assert_ne!(second_identity, original);
            assert_eq!(bytes::snapshot(&f.target_path()), first);
            assert_eq!(bytes::tree(&f.target_root).len(), 2);
            assert_eq!(bytes::tree(&f.source_root), before);
            drop(other_ack);
            drop(assembled);
        })
    })
    .await;
}

#[tokio::test]
async fn two_concurrent_borrowed_original_ack_saves_have_independent_offsets_and_distinct_checked_outputs(
) {
    checked(
        Fixture::with_workers(FILE_TRANSFER_CHUNK_BYTES + 17, 2),
        |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let second_leaf = "parallel-selected-file.bin";
                let second = f.target_for(second_leaf);
                let source = bytes::tree(&f.source_root);
                let results = within(async {
                    tokio::join!(
                        f.store
                            .save_assembled_file(&f.manifest, &assembled, &f.target),
                        f.store
                            .save_assembled_file(&f.manifest, &assembled, &second)
                    )
                })
                .await;
                let first = results.0.unwrap();
                let second = results.1.unwrap();
                assert_eq!(first.file(), &FileVersionRef::from_manifest(&f.manifest));
                assert_eq!(second.file(), first.file());
                let original = observe(&f.source_path(), &f.manifest).unwrap().identity;
                let left = assert_output(f, &f.target_path(), &f.manifest);
                let right = assert_output(f, &f.target_root.join(second_leaf), &f.manifest);
                assert_ne!(left, right);
                assert_ne!(left, original);
                assert_ne!(right, original);
                assert_eq!(bytes::tree(&f.source_root), source);
                assert_eq!(bytes::tree(&f.target_root).len(), 2);
                // These are actual concurrently-polled public calls. Deterministic simultaneous in-job
                // barriers, original-FD cursor observation and cancellation ownership are private tests.
            })
        },
    )
    .await;
}
