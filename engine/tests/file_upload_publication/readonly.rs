use super::{fixture::*, support::*};
use cyanrex_engine::services::file_upload_staging::{
    FileUploadPublicationError, FileUploadPublicationState,
};
use sqlx_core::{query::query, row::Row};

async fn set(f: &Fixture, on: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.publication_pool.acquire().await.unwrap();
        query(if on {
            "SET default_transaction_read_only='on'"
        } else {
            "SET default_transaction_read_only='off'"
        })
        .execute(&mut *c)
        .await
        .unwrap();
        held.push(c);
    }
}
pub async fn exercise() {
    let f = Fixture::ready(0).await;
    set(&f, true).await;
    let before = snapshot_all(&f).await;
    assert_eq!(
        f.publication
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap(),
        None
    );
    let mut attempt = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    assert_eq!(attempt.state(), FileUploadPublicationState::Ready);
    assert_eq!(snapshot_all(&f).await, before);
    assert_eq!(
        f.publisher.publish(&mut attempt).await.unwrap_err(),
        FileUploadPublicationError::Unconfirmed
    );
    assert_eq!(attempt.state(), FileUploadPublicationState::Unknown);
    assert_eq!(snapshot_all(&f).await, before);
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.publication_pool.acquire().await.unwrap();
        assert_eq!(
            query("SELECT current_setting('default_transaction_read_only') AS v")
                .fetch_one(&mut *c)
                .await
                .unwrap()
                .get::<String, _>("v"),
            "on"
        );
        held.push(c);
    }
    drop(held);
    set(&f, false).await;
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
}
