use crate::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
async fn actual_empty_and_short_success_with_live_handle_are_not_retroactively_cancelled_and_private_700_control_is_unchanged(
) {
    for length in [0, 17] {
        checked(
            Fixture::with_manifest(bytes::manifest(length, 1)).await,
            |f| {
                Box::pin(async move {
                    let selected = selected_directory(f);
                    let target = if length == 0 {
                        FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap()
                    } else {
                        fs::set_permissions(&selected, fs::Permissions::from_mode(0o700)).unwrap();
                        FileSaveTarget::open(&selected, CHOSEN_LEAF).unwrap()
                    };
                    let (handle, cancellation) = FileDeliveryCancelHandle::channel();
                    let (observer, mut progress) = FileDeliveryObserver::channel();
                    let saved = controlled(f, &f.client(), &target, observer, cancellation)
                        .await
                        .unwrap();
                    assert_eq!(saved, FileVersionRef::from_manifest(&f.manifest));
                    let count = if length == 0 { 0 } else { 1 };
                    let complete = snapshot(
                        f,
                        &progress,
                        FileDeliveryPhase::Complete,
                        count,
                        length,
                        Some(count),
                    );
                    let before = (bytes::tree(&f.root), bytes::tree(&selected));
                    handle.cancel(); // No active wait remains. This request cannot retract a chosen result.
                    closed(&mut progress, Some(complete)).await;
                    assert_eq!((bytes::tree(&f.root), bytes::tree(&selected)), before);
                    assert_output(f, &selected.join(CHOSEN_LEAF));
                    assert_output(f, &f.root.join(output_name(&f.manifest)));
                    assert_eq!(
                        indices(&f.take_requests()),
                        if length == 0 {
                            vec![None]
                        } else {
                            vec![None, Some(0)]
                        }
                    );
                    assert_eq!(
                        directory_policy(&selected).4,
                        if length == 0 { 0o755 } else { 0o700 }
                    );
                })
            },
        )
        .await;
    }
}
