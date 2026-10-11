use super::{fixture::*, publication::Published, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_metadata_validates_selected_records_and_reservation_full_ledger() {
    let f = Fixture::ready().await;
    let learner = reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
        .await
        .unwrap();
    let other = reserve(&f, &f.auth.manager_token, 2, &manifest(17, 2))
        .await
        .unwrap();
    let original: Vec<u8> = query(&format!(
        "SELECT manifest FROM {}.{UPLOAD_ROWS} WHERE upload_id=$1",
        f.upload_schema
    ))
    .bind(upload(1).as_uuid())
    .fetch_one(&f.auth.base.admin)
    .await
    .unwrap()
    .get("manifest");
    for bad in [b"not-json".to_vec(), [original.as_slice(), b"\n"].concat()] {
        query(&format!(
            "UPDATE {}.{UPLOAD_ROWS} SET manifest=$1 WHERE upload_id=$2",
            f.upload_schema
        ))
        .bind(bad)
        .bind(upload(1).as_uuid())
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
        assert_eq!(
            get_upload(&f, &f.auth.learner_token, 1).await,
            Err(SessionFileMetadataError::Upload(
                FileUploadStoreError::InvalidRecord
            ))
        );
        assert_eq!(
            get_upload(&f, &f.auth.manager_token, 1).await.unwrap(),
            None,
            "owner filtering does not decode another owner's selected body"
        );
        assert_eq!(
            get_upload(&f, &f.auth.manager_token, 2).await.unwrap(),
            Some(other.clone()),
            "GET checks its selected row, not every foreign intent"
        );
        assert_eq!(
            reserve(&f, &f.auth.manager_token, 3, &manifest(17, 3)).await,
            Err(SessionFileMetadataError::Upload(
                FileUploadStoreError::InvalidRecord
            )),
            "writer validates corrupt foreign usage before conflict/quota"
        );
        query(&format!(
            "UPDATE {}.{UPLOAD_ROWS} SET manifest=$1 WHERE upload_id=$2",
            f.upload_schema
        ))
        .bind(&original)
        .bind(upload(1).as_uuid())
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
        assert_eq!(
            get_upload(&f, &f.auth.learner_token, 1).await.unwrap(),
            Some(learner.clone())
        );
    }
    let published = Published::empty(&f, &f.auth.learner_token, 4).await;
    let journal = published.rows(&f).await;
    let raw: Vec<u8> = query(&format!(
        "SELECT manifest FROM {}.{PUBLICATION_ROWS} WHERE upload_id=$1",
        f.publication_schema
    ))
    .bind(upload(4).as_uuid())
    .fetch_one(&f.auth.base.admin)
    .await
    .unwrap()
    .get("manifest");
    let expected = get_publication(&f, &f.auth.learner_token, 4)
        .await
        .unwrap()
        .unwrap();
    for bad in [
        b"{}".to_vec(),
        [raw.as_slice(), b"\n"].concat(),
        serde_json::to_vec(&manifest(0, 5)).unwrap(),
    ] {
        query(&format!(
            "UPDATE {}.{PUBLICATION_ROWS} SET manifest=$1 WHERE upload_id=$2",
            f.publication_schema
        ))
        .bind(bad)
        .bind(upload(4).as_uuid())
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
        assert_eq!(
            get_publication(&f, &f.auth.learner_token, 4).await,
            Err(SessionFileMetadataError::Publication(
                FileUploadPublicationStoreError::InvalidRecord
            ))
        );
        assert_eq!(
            get_publication(&f, &f.auth.manager_token, 4).await.unwrap(),
            None
        );
        query(&format!(
            "UPDATE {}.{PUBLICATION_ROWS} SET manifest=$1 WHERE upload_id=$2",
            f.publication_schema
        ))
        .bind(&raw)
        .bind(upload(4).as_uuid())
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
        assert_eq!(
            get_publication(&f, &f.auth.learner_token, 4).await.unwrap(),
            Some(expected.clone())
        );
    }
    assert_eq!(published.rows(&f).await, journal);
    f.pool_restored().await;
    published.cleanup(&f).await;
    f.cleanup().await;
}
