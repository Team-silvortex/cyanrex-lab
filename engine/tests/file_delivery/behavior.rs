use crate::{fixture::*, support::*, *};

#[tokio::test]
async fn empty_and_short_delivery_require_metadata_and_save_checked_new_file_without_extra_http() {
    for length in [0, 17] {
        checked(
            Fixture::with_manifest(bytes::manifest(length, 1)).await,
            |f| {
                Box::pin(async move {
                    let parent = selected_parent(f);
                    let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                    let client = f.client();
                    assert_eq!(
                        deliver(f, &client, &target).await.unwrap(),
                        FileVersionRef::from_manifest(&f.manifest)
                    );
                    assert_output(f, &parent.join(CHOSEN_LEAF));
                    assert_output(f, &f.root.join(output_name(&f.manifest)));
                    let requests = f.take_requests();
                    let expected = if length == 0 {
                        vec![None]
                    } else {
                        vec![None, Some(0)]
                    };
                    assert_eq!(
                        indices(&requests),
                        expected,
                        "even an empty file must obtain metadata; selected save makes no GET"
                    );
                    for (request, index) in requests.iter().zip(expected) {
                        assert_request(request, &f.manifest, index);
                    }
                    assert_eq!(bytes::tree(&f.root).len(), if length == 0 { 1 } else { 2 });
                    let selected = bytes::tree(&parent);
                    assert_eq!(selected.len(), 1);
                    assert_eq!(selected[0].name, std::ffi::OsString::from(CHOSEN_LEAF));
                    assert!(!parent.join(f.manifest.display_name()).exists());
                })
            },
        )
        .await;
    }
}

#[tokio::test]
async fn repeated_delivery_even_to_another_target_refuses_existing_chunk_or_assembly_instead_of_resuming_save(
) {
    for length in [0, 17] {
        checked(
            Fixture::with_manifest(bytes::manifest(length, 1)).await,
            |f| {
                Box::pin(async move {
                    let parent = selected_parent(f);
                    let target = FileSaveTarget::open(&parent, CHOSEN_LEAF).unwrap();
                    let other = FileSaveTarget::open(&parent, "another-selected.bin").unwrap();
                    let client = f.client();
                    deliver(f, &client, &target).await.unwrap();
                    f.take_requests();
                    let source = bytes::tree(&f.root);
                    let selected = bytes::tree(&parent);
                    for target in [&target, &other] {
                        let error = deliver(f, &client, target).await.unwrap_err();
                        let expected_error = if length == 0 {
                            assembly_error(ChunkStagingError::Conflict)
                        } else {
                            staging_error(0, ChunkStagingError::Conflict)
                        };
                        assert_eq!(error, expected_error,
                        "the one-shot entry never adopts an old chunk or recovers an original ACK");
                        redacted(f, &error);
                        let requests = f.take_requests();
                        assert_eq!(
                            indices(&requests),
                            if length == 0 {
                                vec![None]
                            } else {
                                vec![None, Some(0)]
                            }
                        );
                        assert_eq!(bytes::tree(&f.root), source);
                        assert_eq!(bytes::tree(&parent), selected);
                        assert!(!parent.join("another-selected.bin").exists());
                    }
                    assert_output(f, &parent.join(CHOSEN_LEAF));
                })
            },
        )
        .await;
    }
}
