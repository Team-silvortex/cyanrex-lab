use crate::{fixture::*, support::*, *};
use std::{
    future::{poll_fn, Future},
    sync::Arc,
    task::Poll,
};

#[tokio::test]
async fn cancelling_one_unique_observer_closes_only_its_view_and_another_pair_completes_on_the_same_clone_shared_client(
) {
    checked(Fixture::with_manifest(bytes::manifest(17, 1)).await, |f| {
        Box::pin(async move {
            let selected = selected_directory(f);
            let first_target =
                FileSaveTarget::open_user_directory(&selected, "cancelled-first.bin").unwrap();
            let second_target =
                FileSaveTarget::open_user_directory(&selected, "independent-second.bin").unwrap();
            let client = f.client();
            let expected = FileVersionRef::from_manifest(&f.manifest);
            let gate = Arc::new(tokio::sync::Semaphore::new(0));
            let mut reply = Reply::good(&f.manifest, Kind::Manifest);
            reply.header_gate = Some(gate.clone());
            f.set_reply(Kind::Manifest, reply);
            let (first_observer, mut first) = FileDeliveryObserver::channel();
            let (second_observer, mut second) = FileDeliveryObserver::channel();
            let mut pending = Box::pin(client.fetch_granted_and_save_observed(
                TOKEN,
                grant(),
                upload(),
                &expected,
                &first_target,
                first_observer,
            ));
            drive(f, 1, &mut pending).await;
            let copied = snapshot(f, &first, FileDeliveryPhase::FetchingManifest, 0, 0, None);
            assert!(second.latest().is_none());
            drop(pending);
            closed(&mut first, Some(copied)).await;
            // Poll the separately owned, still-unconsumed pair once. No timer or inferred silence:
            // its sender is live and this actual changed future must remain Pending, not close.
            let mut idle = Box::pin(second.changed());
            let pending_idle = poll_fn(|cx| match idle.as_mut().poll(cx) {
                Poll::Pending => Poll::Ready(true),
                Poll::Ready(_) => Poll::Ready(false),
            })
            .await;
            drop(idle);
            assert!(
                pending_idle,
                "closing one producer cannot end the other reader's owned channel"
            );
            gate.add_permits(1);
            f.clear_reply(Kind::Manifest);
            assert_eq!(
                observed(f, &client.clone(), &second_target, second_observer)
                    .await
                    .unwrap(),
                expected
            );
            let completed = snapshot(f, &second, FileDeliveryPhase::Complete, 1, 17, Some(1));
            closed(&mut second, Some(completed)).await;
            assert_eq!(first.latest(), Some(copied));
            assert_eq!(within(first.changed()).await, None);
            assert!(!selected.join("cancelled-first.bin").exists());
            assert_eq!(bytes::tree(&selected).len(), 1);
            assert_output(f, &selected.join("independent-second.bin"));
            assert_eq!(bytes::tree(&f.root).len(), 2);
            within(f.staging.verify_staged_chunk(&f.manifest, 0))
                .await
                .unwrap();
            assert_eq!(indices(&f.take_requests()), [None, None, Some(0)]);
            assert_eq!(second.latest(), Some(completed));
        })
    })
    .await;
}
