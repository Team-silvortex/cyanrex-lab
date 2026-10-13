use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn two_concurrently_polled_saves_borrow_one_original_ack_into_same_ordinary_parent_without_shared_cursor_or_directory_timestamp_binding(
) {
    checked(
        Fixture::with_workers(FILE_TRANSFER_CHUNK_BYTES + 17, 2),
        |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let selected = selected_directory(f);
                mode(&selected, 0o750);
                let parent = directory_policy(&selected);
                let source = bytes::tree(&f.source_root);
                let left =
                    FileSaveTarget::open_user_directory(&selected, "left-selected.bin").unwrap();
                let right =
                    FileSaveTarget::open_user_directory(&selected, "right-selected.bin").unwrap();
                let results = within(async {
                    tokio::join!(
                        f.store.save_assembled_file(&f.manifest, &assembled, &left),
                        f.store.save_assembled_file(&f.manifest, &assembled, &right)
                    )
                })
                .await;
                assert_eq!(
                    results.0.unwrap().file(),
                    &FileVersionRef::from_manifest(&f.manifest)
                );
                assert_eq!(
                    results.1.unwrap().file(),
                    &FileVersionRef::from_manifest(&f.manifest)
                );
                let left_identity = assert_output(f, &selected.join("left-selected.bin"));
                let right_identity = assert_output(f, &selected.join("right-selected.bin"));
                let original = observe(&f.source_path(), &f.manifest).unwrap().identity;
                assert_ne!(left_identity, right_identity);
                assert_ne!(left_identity, original);
                assert_ne!(right_identity, original);
                assert_eq!(directory_policy(&selected), parent);
                assert_eq!(bytes::tree(&f.source_root), source);
                assert_eq!(bytes::tree(&selected).len(), 2);
                let entries = bytes::tree(&selected);
                assert_eq!(
                    save(f, &assembled, &left).await.err(),
                    Some(FileSaveError::Conflict)
                );
                assert_eq!(bytes::tree(&selected), entries);
                // Real public calls are concurrently polled. Simultaneous in-job phases/cancellation and
                // original-FD cursor observations belong to the private deterministic barrier tests.
            })
        },
    )
    .await;
}
