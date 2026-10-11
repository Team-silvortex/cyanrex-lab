use super::{fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_publication::FileUploadPublicationStoreError,
    file_upload_staging::{
        FileUploadPublicationError, FileUploadPublicationState, FileUploadPublisher,
    },
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cross_chunk_empty_publication_and_metadata_reads_are_exact() {
    for length in [
        0,
        cyanrex_engine::models::collaboration::FILE_TRANSFER_CHUNK_BYTES + 17,
    ] {
        let f = Fixture::ready(length).await;
        stage_all(&f).await;
        let initial = snapshot_all(&f).await;
        let mut attempt = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        let independent =
            FileUploadPublisher::new(f.local.clone(), f.journal.clone(), f.publication.clone())
                .unwrap();
        assert_eq!(
            independent.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::InvalidInput
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Ready);
        assert_eq!(snapshot_all(&f).await, initial);
        receipt_metadata(
            &f.publisher.clone().publish(&mut attempt).await.unwrap(),
            1,
            &f.manifest,
        );
        assert_eq!(attempt.state(), FileUploadPublicationState::Confirmed);
        let committed = snapshot_all(&f).await;
        assert_eq!(committed.rows[..2], initial.rows[..2]);
        assert_eq!(committed.oids, initial.oids);
        assert_eq!(committed.rows[2][0], initial.rows[2][0]);
        assert_eq!(committed.rows[2][1].len(), 1);
        assert_eq!(committed.files, initial.files);
        assert_eq!(committed.root, initial.root);
        let before = snapshot_all(&f).await;
        let (store, pool) = reopen(&f).await;
        record_metadata(
            &store
                .get(upload(1), owner(1), account(1))
                .await
                .unwrap()
                .unwrap(),
            1,
            &f.manifest,
        );
        assert_eq!(
            store.get(upload(1), owner(2), account(1)).await.unwrap(),
            None
        );
        assert_eq!(
            store.get(upload(1), owner(1), account(2)).await.unwrap(),
            None
        );
        assert_eq!(
            store.get(upload(9), owner(1), account(1)).await.unwrap(),
            None
        );
        let mut foreign = owner(1);
        foreign.authority_id = uuid::Uuid::new_v4().try_into().unwrap();
        assert_eq!(
            store.get(upload(1), foreign, account(1)).await,
            Err(FileUploadPublicationStoreError::ScopeMismatch)
        );
        assert_eq!(snapshot_all(&f).await, before);
        let mut duplicate = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        assert_eq!(
            f.publisher.publish(&mut duplicate).await.unwrap_err(),
            FileUploadPublicationError::Publication(FileUploadPublicationStoreError::Conflict)
        );
        assert_eq!(duplicate.state(), FileUploadPublicationState::Unknown);
        assert_eq!(snapshot_all(&f).await, before);
        pool.close().await;
        f.cleanup().await;
    }
    let mut f = Fixture::new(1, 2, 16, 8).await;
    f.journal.install_empty_namespace().await.unwrap();
    f.publication.install_empty_namespace().await.unwrap();
    f.manifest = manifest(17, 1);
    f.source
        .reserve(upload(1), owner(1), account(1), &f.manifest)
        .await
        .unwrap();
    stage_all(&f).await;
    let mut admitted = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    let mut refused = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    let before = snapshot_all(&f).await;
    let (mut holder, pid) = lock(&f, Store::Source).await;
    let mut pending = Box::pin(f.publisher.publish(&mut admitted));
    tokio::select! {result=&mut pending=>panic!("source admission never reached true SQL wait: {result:?}"),_=f.wait_blocked(Store::Source,pid)=>{}}
    assert!(matches!(
        f.publisher
            .clone()
            .prepare(upload(1), owner(1), account(1))
            .await,
        Err(FileUploadPublicationError::CapacityExhausted)
    ));
    assert_eq!(
        f.publisher.clone().publish(&mut refused).await.unwrap_err(),
        FileUploadPublicationError::CapacityExhausted
    );
    assert_eq!(refused.state(), FileUploadPublicationState::Ready);
    // Snapshot source through its lock owner, so proving nonmutation cannot deadlock itself.
    let (source, oids) = state_in_tx(&f, Store::Source, &mut holder).await;
    assert_eq!(source, before.rows[0]);
    assert_eq!(oids, before.oids[0]);
    assert_eq!(f.rows(Store::Journal).await, before.rows[1]);
    assert_eq!(f.rows(Store::Publication).await, before.rows[2]);
    assert_eq!(super::bytes::tree(&f.root), before.files);
    assert_eq!(super::bytes::snapshot(&f.root), before.root);
    drop(pending);
    holder.rollback().await.unwrap();
    assert_eq!(admitted.state(), FileUploadPublicationState::Unknown);
    f.pool_restored(Store::Source).await;
    assert_eq!(snapshot_all(&f).await, before);
    receipt_metadata(
        &f.publisher.clone().publish(&mut refused).await.unwrap(),
        1,
        &f.manifest,
    );
    assert_eq!(refused.state(), FileUploadPublicationState::Confirmed);
    f.cleanup().await;
    super::readonly::exercise().await;
}
