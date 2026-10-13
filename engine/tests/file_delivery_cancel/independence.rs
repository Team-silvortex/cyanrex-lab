use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn cancelling_one_of_two_unique_pairs_does_not_cancel_the_other_shared_client_flow_or_old_api_control(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let first_target =
                FileSaveTarget::open_user_directory(&selected, "cancelled-first.bin").unwrap();
            let second_target =
                FileSaveTarget::open_user_directory(&selected, "healthy-second.bin").unwrap();
            let client = FileChunkClient::new_loopback(&f.origin, f.staging.clone()).unwrap();
            let clone = client.clone();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let (first_handle, first_cancellation) = FileDeliveryCancelHandle::channel();
            let (first_observer, mut first_view) = FileDeliveryObserver::channel();
            let mut first = Box::pin(client.fetch_granted_and_save_controlled(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &first_target,
                first_observer,
                first_cancellation,
            ));
            drive(f, 1, &mut first).await;
            let (second_handle, second_cancellation) = FileDeliveryCancelHandle::channel();
            let (second_observer, mut second_view) = FileDeliveryObserver::channel();
            let mut second = Box::pin(clone.fetch_granted_and_save_controlled(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &second_target,
                second_observer,
                second_cancellation,
            ));
            drive(f, 2, &mut second).await;
            let first_last = snapshot(
                f,
                &first_view,
                FileDeliveryPhase::FetchingManifest,
                0,
                0,
                None,
            );
            snapshot(
                f,
                &second_view,
                FileDeliveryPhase::FetchingManifest,
                0,
                0,
                None,
            );
            first_handle.cancel();
            assert_cancelled(
                within(first)
                    .await
                    .err()
                    .expect("selected first cancellation"),
            );
            closed(&mut first_view, Some(first_last)).await;
            f.clear_reply(Kind::Manifest); // Both accepted held replies were already independently frozen.
            gate.add_permits(2);
            assert_eq!(within(second).await.unwrap().file(), &expected);
            drop(second_handle);
            let complete = snapshot(f, &second_view, FileDeliveryPhase::Complete, 1, 17, Some(1));
            closed(&mut second_view, Some(complete)).await;
            assert_eq!(first_view.latest(), Some(first_last));
            assert!(!selected.join("cancelled-first.bin").exists());
            assert_output(f, &selected.join("healthy-second.bin"));
            assert_eq!(bytes::tree(&selected).len(), 1);
            assert_eq!(
                plain(f, &client, &second_target).await.err(),
                Some(staging_conflict())
            );
            assert_eq!(first_view.latest(), Some(first_last));
            assert_eq!(second_view.latest(), Some(complete));
            assert_eq!(
                indices(&f.take_requests()),
                [None, None, Some(0), None, Some(0)]
            );
            within(f.staging.verify_staged_chunk(&f.manifest, 0))
                .await
                .unwrap();
        })
    })
    .await;
}
