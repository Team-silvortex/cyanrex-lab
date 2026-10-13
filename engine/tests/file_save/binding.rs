use crate::{fixture::*, support::*, *};
use serde_json::json;

#[tokio::test]
async fn save_rejects_scope_version_whole_and_complete_manifest_binding_without_output_or_source_changes(
) {
    checked(Fixture::new(), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let mut authority = scope();
            authority.authority_id = Uuid::from_u128(1301).try_into().unwrap();
            let mut workspace = scope();
            workspace.workspace_id = Uuid::from_u128(1302).try_into().unwrap();
            let mut chunks = f.manifest.chunk_sha256().to_vec();
            chunks.reverse();
            let variants = [
                changed(&f.manifest, "workspace", json!(authority)),
                changed(&f.manifest, "workspace", json!(workspace)),
                changed(&f.manifest, "file_id", json!(Uuid::from_u128(1303))),
                changed(&f.manifest, "version_id", json!(Uuid::from_u128(1304))),
                changed(
                    &f.manifest,
                    "byte_length",
                    json!(f.manifest.byte_length() + 1),
                ),
                changed(&f.manifest, "sha256", json!("a".repeat(64))),
                changed(
                    &f.manifest,
                    "display_name",
                    json!("same-version-different-display.bin"),
                ),
                changed(&f.manifest, "chunk_sha256", json!(chunks)),
            ];
            let before = bytes::tree(&f.source_root);
            for manifest in variants {
                assert_eq!(
                    within(
                        f.store
                            .save_assembled_file(&manifest, &assembled, &f.target)
                    )
                    .await
                    .err(),
                    Some(FileSaveError::InvalidInput)
                );
                assert_eq!(bytes::tree(&f.source_root), before);
                assert!(bytes::tree(&f.target_root).is_empty());
            }
            assert_eq!(
                save(f, &assembled, &f.target).await.unwrap(),
                FileVersionRef::from_manifest(&f.manifest)
            );
            assert_output(f, &f.target_path(), &f.manifest);
            assert_eq!(bytes::tree(&f.source_root), before);
        })
    })
    .await;
}

#[tokio::test]
async fn save_requires_original_receipt_root_and_distinct_target_root_not_matching_metadata_from_another_ack(
) {
    checked(Fixture::with_length(17), |f| {
        Box::pin(async move {
            let assembled = f.assemble().await;
            let other_manifest = bytes::manifest(17, 2);
            let other_ack = f.assemble_manifest(&other_manifest).await;
            let unrelated_root = f.directory().join("other-source-root");
            private_directory(&unrelated_root);
            let independent = ChunkStaging::open(scope(), &unrelated_root, 1).unwrap();
            let same_root_target =
                FileSaveTarget::open(&f.source_root, "must-not-create.bin").unwrap();
            let source = bytes::tree(&f.source_root);
            assert_eq!(
                within(
                    f.store
                        .save_assembled_file(&f.manifest, &other_ack, &f.target)
                )
                .await
                .err(),
                Some(FileSaveError::InvalidInput)
            );
            assert_eq!(
                within(independent.save_assembled_file(&f.manifest, &assembled, &f.target))
                    .await
                    .err(),
                Some(FileSaveError::InvalidInput)
            );
            assert_eq!(
                within(
                    f.store
                        .save_assembled_file(&f.manifest, &assembled, &same_root_target)
                )
                .await
                .err(),
                Some(FileSaveError::InvalidInput)
            );
            assert!(bytes::tree(&unrelated_root).is_empty());
            assert!(bytes::tree(&f.target_root).is_empty());
            assert_eq!(bytes::tree(&f.source_root), source);
            assert_eq!(
                save(f, &assembled, &f.target).await.unwrap(),
                FileVersionRef::from_manifest(&f.manifest)
            );
            assert_output(f, &f.target_path(), &f.manifest);
            assert_eq!(bytes::tree(&f.source_root), source);
        })
    })
    .await;
}
