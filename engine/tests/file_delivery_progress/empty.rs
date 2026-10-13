use crate::{fixture::*, support::*, *};
use std::fs;

#[tokio::test]
async fn zero_of_zero_is_not_complete_until_actual_empty_save_ack_and_save_conflict_remains_saving()
{
    for conflict in [false, true] {
        checked(Fixture::with_manifest(bytes::manifest(0, 1)).await, |f| {
            Box::pin(async move {
                let selected = selected_directory(f);
                if conflict {
                    fs::write(
                        selected.join(CHOSEN_LEAF),
                        b"unchanged empty-call conflict marker",
                    )
                    .unwrap();
                }
                let before = bytes::tree(&selected);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let (observer, mut progress) = FileDeliveryObserver::channel();
                let result = observed(f, &f.client(), &target, observer).await;
                let phase = if conflict {
                    assert_eq!(
                        result.err(),
                        Some(FileGrantedSaveError::Save(FileSaveError::Conflict))
                    );
                    assert_eq!(bytes::tree(&selected), before);
                    FileDeliveryPhase::Saving
                } else {
                    assert_eq!(result.unwrap(), FileVersionRef::from_manifest(&f.manifest));
                    assert_output(f, &selected.join(CHOSEN_LEAF));
                    assert_eq!(bytes::tree(&selected).len(), 1);
                    FileDeliveryPhase::Complete
                };
                let last = snapshot(f, &progress, phase, 0, 0, Some(0));
                closed(&mut progress, Some(last)).await;
                assert_eq!(
                    indices(&f.take_requests()),
                    [None],
                    "even empty metadata is fetched; no chunk GET or progress unit is invented"
                );
                assert_eq!(bytes::tree(&f.root).len(), 1);
                assert_output(f, &f.root.join(output_name(&f.manifest)));
            })
        })
        .await;
    }
}
