use crate::{fixture::*, support::*, *};
use std::sync::Arc;

#[tokio::test]
async fn dropping_unique_handle_before_poll_or_during_actual_manifest_wait_does_not_cancel_or_block_healthy_delivery(
) {
    for before_poll in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
            Box::pin(async move {
                let selected = selected_directory(f);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let client = f.client();
                let expected = FileVersionRef::from_manifest(&f.manifest);
                let gate = Arc::new(tokio::sync::Semaphore::new(0));
                let mut reply = Reply::good(&f.manifest, Kind::Manifest);
                reply.header_gate = Some(gate.clone());
                f.set_reply(Kind::Manifest, reply);
                let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                let mut handle = Some(handle);
                if before_poll {
                    drop(handle.take());
                }
                let (observer, mut progress) = FileDeliveryObserver::channel();
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
                snapshot(
                    f,
                    &progress,
                    FileDeliveryPhase::FetchingManifest,
                    0,
                    0,
                    None,
                );
                drop(handle.take());
                gate.add_permits(1);
                assert_eq!(within(pending).await.unwrap().file(), &expected);
                let last = snapshot(f, &progress, FileDeliveryPhase::Complete, 1, 17, Some(1));
                closed(&mut progress, Some(last)).await;
                assert_output(f, &selected.join(CHOSEN_LEAF));
                assert_eq!(indices(&f.take_requests()), [None, Some(0)]);
            })
        })
        .await;
    }
}
