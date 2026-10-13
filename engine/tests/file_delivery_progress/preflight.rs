use crate::{fixture::*, support::*, *};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    sync::Arc,
};

#[tokio::test]
async fn unpolled_invalid_and_full_gate_calls_close_without_events_while_pure_input_priority_preserves_the_running_view(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
            let same_source =
                FileSaveTarget::open_user_directory(&f.root, "forbidden-source-target.bin")
                    .unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let (observer, mut unpolled) = FileDeliveryObserver::channel();
            drop(client.fetch_granted_and_save_observed(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &target,
                observer,
            ));
            assert!(unpolled.latest().is_none());
            closed(&mut unpolled, None).await;
            assert_eq!(f.request_count(), 0);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let (observer, mut running) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_observed(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &target,
                observer,
            ));
            drive(f, 1, &mut pending).await;
            let current = snapshot(f, &running, FileDeliveryPhase::FetchingManifest, 0, 0, None);
            for token in [
                "bad-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
            ] {
                let (observer, mut progress) = FileDeliveryObserver::channel();
                assert_eq!(
                    within(client.fetch_granted_and_save_observed(
                        token,
                        grant(),
                        upload(),
                        &expected,
                        &target,
                        observer
                    ))
                    .await
                    .err(),
                    Some(FileGrantedSaveError::InvalidInput)
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
                let (observer, mut progress) = FileDeliveryObserver::channel();
                assert_eq!(
                    within(client.fetch_granted_and_save_observed(
                        TOKEN,
                        grant(),
                        upload(),
                        &FileVersionRef::from_manifest(&manifest),
                        &target,
                        observer
                    ))
                    .await
                    .err(),
                    Some(FileGrantedSaveError::InvalidInput)
                );
                closed(&mut progress, None).await;
            }
            let (observer, mut rejected) = FileDeliveryObserver::channel();
            assert_eq!(
                observed(f, &client, &same_source, observer).await.err(),
                Some(FileGrantedSaveError::InvalidInput)
            );
            closed(&mut rejected, None).await;
            let (observer, mut saturated) = FileDeliveryObserver::channel();
            assert_eq!(
                observed(f, &client.clone(), &target, observer).await.err(),
                Some(FileGrantedSaveError::CapacityExhausted)
            );
            closed(&mut saturated, None).await;
            assert_eq!(
                plain(f, &client, &target).await.err(),
                Some(FileGrantedSaveError::CapacityExhausted)
            );
            assert_eq!(
                within(client.fetch_granted_and_assemble(TOKEN, grant(), upload(), &expected))
                    .await
                    .err(),
                Some(FileGrantedDownloadError::CapacityExhausted)
            );
            assert_eq!(f.request_count(), 1);
            assert_eq!(running.latest(), Some(current));
            assert!(bytes::tree(&f.root).is_empty() && bytes::tree(&selected).is_empty());
            drop(pending);
            closed(&mut running, Some(current)).await;
            gate.add_permits(1);
            f.clear_reply(Kind::Manifest);
            let private = f
                .root
                .parent()
                .unwrap()
                .join("default-private-observed-target");
            fs::DirBuilder::new().mode(0o700).create(&private).unwrap();
            fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
            let private_target = FileSaveTarget::open(&private, "private-control.bin").unwrap();
            let (observer, mut control) = FileDeliveryObserver::channel();
            observed(f, &client, &private_target, observer)
                .await
                .unwrap();
            let complete = snapshot(f, &control, FileDeliveryPhase::Complete, 1, 17, Some(1));
            closed(&mut control, Some(complete)).await;
            assert_output(f, &private.join("private-control.bin"));
            assert_eq!(directory_policy(&private).4, 0o700);
            assert!(bytes::tree(&selected).is_empty());
            assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
        })
    })
    .await;
}
