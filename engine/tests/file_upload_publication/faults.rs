use super::{fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_publication::FileUploadPublicationStoreError,
    file_upload_staging::{FileUploadPublicationError, FileUploadPublicationState},
};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_insert_and_deferred_foreign_delta_faults_never_adopt_rows() {
    for failure in ["suppressed", "raised", "foreign", "candidate"] {
        let f = Fixture::ready(17).await;
        stage_all(&f).await;
        healthy(&f, 2).await;
        let mut attempt = f
            .publisher
            .prepare(upload(1), owner(1), account(1))
            .await
            .unwrap();
        let body=match failure {
            "suppressed"=>"RETURN NULL;".to_owned(),
            "raised"=>"RAISE EXCEPTION 'explicit publication insert failure';".to_owned(),
            "foreign"=>format!("UPDATE {PUBLICATION_ROWS} SET owner_id='{}'::uuid WHERE upload_id='{}'::uuid;",
                owner(3).principal_id.as_uuid(),upload(2).as_uuid()),
            "candidate"=>format!("UPDATE {PUBLICATION_ROWS} SET account_id='{}'::uuid WHERE upload_id=NEW.upload_id;",account(3).as_uuid()),
            _=>unreachable!()
        };
        fault(&f, &body, matches!(failure, "foreign" | "candidate")).await;
        let before = snapshot_all(&f).await;
        let error = f.publisher.publish(&mut attempt).await.unwrap_err();
        let expected = if failure == "raised" {
            FileUploadPublicationError::Unconfirmed
        } else {
            FileUploadPublicationError::Publication(FileUploadPublicationStoreError::InvalidRecord)
        };
        assert_eq!(error, expected, "{failure}");
        assert!(hit(&f).await);
        assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
        assert_eq!(
            snapshot_all(&f).await,
            before,
            "rollback must retain the whole foreign row and original file snapshots"
        );
        assert!(f
            .publication
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_none());
        record_metadata(
            &f.publication
                .get(upload(2), owner(2), account(2))
                .await
                .unwrap()
                .unwrap(),
            2,
            &manifest(17, 2),
        );
        assert_eq!(snapshot_all(&f).await, before);
        remove_fault(&f).await;
        assert_eq!(
            f.publisher.publish(&mut attempt).await.unwrap_err(),
            FileUploadPublicationError::InvalidInput
        );
        assert_eq!(snapshot_all(&f).await, before);
        // A fresh explicitly trusted call is permitted only because the fixture proved no INSERT.
        publish(&f, &f.publisher, 1, &f.manifest).await;
        f.pool_restored(Store::Publication).await;
        f.cleanup().await;
    }
}
