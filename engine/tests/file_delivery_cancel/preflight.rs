use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn valid_prerequest_cancel_and_unpolled_controlled_future_emit_no_progress_http_or_files() {
    for length in [0, 17] {
        checked(
            Fixture::with_manifest(bytes::manifest(length, 1)).await,
            |f| {
                Box::pin(async move {
                    let selected = selected_directory(f);
                    let target =
                        FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                    let client = f.client();
                    let expected = FileVersionRef::from_manifest(&f.manifest);
                    let roots = (directory_policy(&f.root), directory_policy(&selected));
                    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                    let (observer, mut unpolled) = FileDeliveryObserver::channel();
                    handle.cancel();
                    drop(client.fetch_granted_and_save_controlled(
                        TOKEN,
                        grant(),
                        upload(),
                        &expected,
                        &target,
                        observer,
                        cancellation,
                    ));
                    closed(&mut unpolled, None).await;
                    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                    let (observer, mut progress) = FileDeliveryObserver::channel();
                    handle.cancel();
                    assert_cancelled(
                        controlled(f, &client, &target, observer, cancellation)
                            .await
                            .unwrap_err(),
                    );
                    closed(&mut progress, None).await;
                    assert_eq!(f.request_count(), 0);
                    assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
                    assert_eq!(
                        (directory_policy(&f.root), directory_policy(&selected)),
                        roots
                    );
                })
            },
        )
        .await;
    }
}

#[tokio::test]
async fn pure_invalid_input_precedes_cancel_and_full_flow_while_valid_prerequest_cancel_precedes_capacity(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let same_source =
                FileSaveTarget::open_user_directory(&f.root, "never-created.bin").unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let (running_handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut running) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_controlled(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &target,
                observer,
                cancellation,
            ));
            drive(f, 1, &mut pending).await;
            let last = snapshot(f, &running, FileDeliveryPhase::FetchingManifest, 0, 0, None);
            for token in [
                "bad-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
            ] {
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                handle.cancel();
                assert_eq!(
                    within(client.fetch_granted_and_save_controlled(
                        token,
                        grant(),
                        upload(),
                        &expected,
                        &target,
                        observer,
                        cancellation
                    ))
                    .await
                    .err(),
                    Some(wrapped(FileGrantedSaveError::InvalidInput))
                );
                closed(&mut progress, None).await;
            }
            for authority in [false, true] {
                let mut wrong = scope();
                if authority {
                    wrong.authority_id = Uuid::from_u128(0xab).try_into().unwrap();
                } else {
                    wrong.workspace_id = Uuid::from_u128(0xab).try_into().unwrap();
                }
                let manifest = changed(&f.manifest, "workspace", serde_json::json!(wrong));
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                handle.cancel();
                assert_eq!(
                    within(client.fetch_granted_and_save_controlled(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&manifest),
                        &target,
                        observer,
                        cancellation
                    ))
                    .await
                    .err(),
                    Some(wrapped(FileGrantedSaveError::InvalidInput))
                );
                closed(&mut progress, None).await;
            }
            for same_root in [true, false] {
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                handle.cancel();
                let error = controlled(
                    f,
                    &client,
                    if same_root { &same_source } else { &target },
                    observer,
                    cancellation,
                )
                .await
                .unwrap_err();
                assert_eq!(
                    error,
                    if same_root {
                        wrapped(FileGrantedSaveError::InvalidInput)
                    } else {
                        FileControlledSaveError::Cancelled
                    }
                );
                closed(&mut progress, None).await;
            }
            let (handle, cancellation) = FileDeliveryCancelHandle::channel();
            let (observer, mut saturated) = FileDeliveryObserver::channel();
            assert_eq!(
                controlled(f, &client.clone(), &target, observer, cancellation)
                    .await
                    .err(),
                Some(wrapped(FileGrantedSaveError::CapacityExhausted))
            );
            drop(handle);
            closed(&mut saturated, None).await;
            assert_eq!(
                plain(f, &client, &target).await.err(),
                Some(FileGrantedSaveError::CapacityExhausted)
            );
            assert_eq!(f.request_count(), 1);
            assert_eq!(running.latest(), Some(last));
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
            running_handle.cancel();
            assert_cancelled(
                within(pending)
                    .await
                    .err()
                    .expect("selected explicit request must return Cancelled"),
            );
            closed(&mut running, Some(last)).await;
            gate.add_permits(1);
            f.clear_reply(Kind::Manifest);
            let (observer, mut control) = FileDeliveryObserver::channel();
            observed(f, &client, &target, observer).await.unwrap();
            let complete = snapshot(f, &control, FileDeliveryPhase::Complete, 1, 17, Some(1));
            closed(&mut control, Some(complete)).await;
            assert_output(f, &selected.join(CHOSEN_LEAF));
            assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
        })
    })
    .await;
}
