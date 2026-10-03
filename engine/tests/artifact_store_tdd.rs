#![cfg(unix)]
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        task_store::{TaskDraft, TaskStore},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

#[path = "artifact_store/faults.rs"]
mod faults;
#[path = "artifact_store/fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn artifact_drafts_are_bounded_and_domain_neutral() {
    assert!(ArtifactDraft::new(kind(), "Empty", "text/plain", vec![]).is_ok());
    assert!(ArtifactDraft::new(kind(), "Binary", "application/octet-stream", vec![0, 255]).is_ok());
    for title in ["".into(), "\t".into(), "x".repeat(257), "a\nb".into()] {
        assert!(ArtifactDraft::new(kind(), title, "text/plain", vec![]).is_err());
    }
    for media in [
        "text",
        "Text/Plain",
        "text/plain; charset=utf-8",
        "text/../plain",
        "text/\nplain",
    ] {
        assert!(ArtifactDraft::new(kind(), "Draft", media, vec![]).is_err());
    }
    assert!(ArtifactDraft::new(kind(), "Large", "text/plain", vec![0; 1_048_577]).is_err());
}

#[tokio::test]
async fn unavailable_artifact_database_never_writes_blobs_or_falls_back() {
    let directory = private_directory();
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = ArtifactStore::open(pool, scope(), &directory).unwrap();
    assert_eq!(
        store.install_empty_namespace().await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.create(id(), id(), owner(), draft(b"Private")).await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn artifact_store_rejects_unsafe_root_without_changing_permissions() {
    use std::os::unix::fs::symlink;
    let directory = private_directory();
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(ArtifactStore::open(pool.clone(), scope(), &directory).is_err());
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o755
    );
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let link = directory.join("alias");
    symlink(&directory, &link).unwrap();
    assert!(ArtifactStore::open(pool.clone(), scope(), &link).is_err());
    assert!(ArtifactStore::open(pool, scope(), &directory.join("missing")).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn artifact_service_does_not_import_teaching_or_join_live_state() {
    assert!(!include_str!("../src/state.rs").contains("ArtifactStore"));
    for file in [
        "mod.rs",
        "commands.rs",
        "records.rs",
        "blob.rs",
        "schema.rs",
        "draft.rs",
    ] {
        let source = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src/services/artifact_store")
                .join(file),
        )
        .unwrap();
        for forbidden in [
            "::domain_packs",
            "::learning",
            "ebpf",
            "AuthRole",
            "AuthService",
            "std::env",
        ] {
            assert!(!source.contains(forbidden), "{file}: {forbidden}");
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_preserve_exact_bytes_and_immutable_revisions_after_reopen() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"old\0\xff"))
        .await
        .unwrap();
    assert_eq!(first.reference.sha256.to_string(), digest(b"old\0\xff"));
    assert_eq!(first.byte_length, 5);
    assert_eq!(u64::from(first.sequence), 1);
    assert!(first.parent.is_none());
    let next = f
        .store
        .revise(&first.reference, id(), owner(), draft(b"new"))
        .await
        .unwrap();
    assert_eq!(next.parent, Some(first.reference.clone()));
    assert_eq!(u64::from(next.sequence), 2);
    let reopened = ArtifactStore::open(f.pool.clone(), scope(), &f.directory).unwrap();
    assert_eq!(
        reopened
            .read(&first.reference, owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"old\0\xff"
    );
    assert_eq!(
        reopened
            .read(&next.reference, owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"new"
    );
    assert_eq!(f.files(), 2);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_owner_scope_and_digest_are_not_interchangeable() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"private"))
        .await
        .unwrap();
    let another = PrincipalRef {
        principal_id: id(),
        ..owner()
    };
    assert_eq!(f.store.read(&first.reference, another).await.unwrap(), None);
    assert_eq!(
        f.store
            .revise(&first.reference, id(), another, draft(b"bad"))
            .await,
        Err(ArtifactStoreError::NotFound)
    );
    let mut wrong = first.reference.clone();
    wrong.sha256 = digest(b"other").parse().unwrap();
    assert_eq!(
        f.store.read(&wrong, owner()).await,
        Err(ArtifactStoreError::InvalidRecord)
    );
    wrong.workspace.workspace_id = id();
    assert_eq!(
        f.store.read(&wrong, owner()).await,
        Err(ArtifactStoreError::ScopeMismatch)
    );
    let foreign = PrincipalRef {
        authority_id: id(),
        ..owner()
    };
    assert_eq!(
        f.store.create(id(), id(), foreign, draft(b"bad")).await,
        Err(ArtifactStoreError::ScopeMismatch)
    );
    let other = f
        .store
        .create(id(), id(), another, draft(b"private"))
        .await
        .unwrap();
    assert_eq!(first.reference.sha256, other.reference.sha256);
    assert_eq!(f.store.read(&other.reference, owner()).await.unwrap(), None);
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_concurrent_writes_and_stale_parents_have_one_winner() {
    let f = Fixture::ready().await;
    let artifact = id();
    let (a, b) = tokio::join!(
        f.store.create(artifact, id(), owner(), draft(b"a")),
        f.store.create(artifact, id(), owner(), draft(b"b"))
    );
    let first = match (a, b) {
        (Ok(winner), Err(ArtifactStoreError::Conflict))
        | (Err(ArtifactStoreError::Conflict), Ok(winner)) => winner,
        other => panic!("unexpected concurrent create: {other:?}"),
    };
    let (a, b) = tokio::join!(
        f.store.revise(&first.reference, id(), owner(), draft(b"c")),
        f.store.revise(&first.reference, id(), owner(), draft(b"d"))
    );
    assert!(matches!(
        (a, b),
        (Ok(_), Err(ArtifactStoreError::StaleRevision))
            | (Err(ArtifactStoreError::StaleRevision), Ok(_))
    ));
    assert_eq!(f.files(), 2);
    assert_eq!(f.count("collaboration_artifact_revisions").await, 2);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_task_inputs_keep_original_content_after_new_revision() {
    let f = Fixture::ready().await;
    let original = f
        .store
        .create(id(), id(), owner(), draft(b"Review this document"))
        .await
        .unwrap();
    let (tasks, pool) = f.task_store().await;
    let task = tasks
        .create(
            id(),
            owner(),
            TaskDraft::manual("Review", vec![original.reference.clone()]).unwrap(),
        )
        .await
        .unwrap();
    let new = f
        .store
        .revise(&original.reference, id(), owner(), draft(b"A later edit"))
        .await
        .unwrap();
    let saved = tasks.get(task.reference, owner()).await.unwrap().unwrap();
    assert_ne!(saved.input_refs[0], new.reference);
    assert_eq!(
        f.store
            .read(&saved.input_refs[0], owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Review this document"
    );
    pool.close().await;
    f.cleanup().await;
}
