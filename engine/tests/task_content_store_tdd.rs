//! Trusted schema-3 storage only: no Session, Artifact bytes, browser or migration acceptance.
use cyanrex_engine::{
    models::collaboration::*,
    services::task_store::{
        TaskContentSnapshot, TaskContentStore, TaskDraft, TaskStore, TaskStoreError,
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};

#[path = "task_content_store/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "task_content_store/behavior.rs"]
mod behavior;
#[path = "task_content_store/concurrency.rs"]
mod concurrency;
#[path = "task_content_store/faults.rs"]
mod faults;

#[tokio::test]
async fn closed_content_store_never_falls_back_or_installs_on_read() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = TaskContentStore::new(pool, scope());
    assert_eq!(
        store.install_empty_namespace().await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.get(reference(), owner()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.create(reference().task_id, owner(), manifest()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .replace(reference(), owner(), rev(1), manifest())
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
}

#[tokio::test]
async fn content_store_checks_explicit_scope_without_using_an_unavailable_database() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = TaskContentStore::new(pool, scope());
    let foreign = PrincipalRef {
        authority_id: id(),
        ..owner()
    };
    assert_eq!(
        store.create(id(), foreign, manifest()).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(
        store.get(reference(), foreign).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(
        store
            .replace(reference(), foreign, rev(1), manifest())
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(
        store
            .transition(reference(), foreign, rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
}
