use super::*;
use std::os::unix::fs::symlink;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_rewound_heads_cannot_hide_newer_published_revisions() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"old"))
        .await
        .unwrap();
    f.store
        .revise(&first.reference, id(), owner(), draft(b"new"))
        .await
        .unwrap();
    query("UPDATE collaboration_artifacts SET head_revision_id = $1, sequence = 1")
        .bind(first.reference.revision_id.as_uuid())
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        f.store.read(&first.reference, owner()).await,
        Err(ArtifactStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .revise(&first.reference, id(), owner(), draft(b"bad"))
            .await,
        Err(ArtifactStoreError::InvalidRecord)
    );
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_bound_persisted_payloads_and_reject_unsupported_contracts() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"old"))
        .await
        .unwrap();
    let original = serde_json::to_string(&first).unwrap();
    f.sql("ALTER TABLE collaboration_artifact_revisions DROP CONSTRAINT collaboration_artifact_revisions_snapshot_check").await;
    for raw in [
        "x".repeat(16_385),
        original.replace("\"schema_version\":1", "\"schema_version\":99"),
        original.replace("\"title\":", "\"unknown\":true,\"title\":"),
    ] {
        query("UPDATE collaboration_artifact_revisions SET snapshot = $1")
            .bind(&raw)
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            f.store.read(&first.reference, owner()).await,
            Err(ArtifactStoreError::InvalidRecord)
        );
    }
    query("UPDATE collaboration_artifact_revisions SET snapshot = $1")
        .bind(original)
        .execute(&f.pool)
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_artifact_outbox DROP CONSTRAINT collaboration_artifact_outbox_snapshot_check").await;
    f.sql("UPDATE collaboration_artifact_outbox SET snapshot = repeat('x', 16385)")
        .await;
    assert_eq!(
        f.store.read(&first.reference, owner()).await,
        Err(ArtifactStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_install_only_empty_namespaces_and_pin_scope() {
    let f = Fixture::new().await;
    assert!(f
        .store
        .create(id(), id(), owner(), draft(b"bad"))
        .await
        .is_err());
    assert_eq!(f.files(), 0);
    f.sql("CREATE TABLE unrelated (value TEXT)").await;
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(ArtifactStoreError::NamespaceNotEmpty)
    );
    f.sql("DROP TABLE unrelated").await;
    f.store.install_empty_namespace().await.unwrap();
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(ArtifactStoreError::NamespaceNotEmpty)
    );
    let another = ArtifactStore::open(
        f.pool.clone(),
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        &f.directory,
    )
    .unwrap();
    assert_eq!(
        another.create(id(), id(), owner(), draft(b"bad")).await,
        Err(ArtifactStoreError::ScopeMismatch)
    );
    f.sql("UPDATE collaboration_artifact_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.create(id(), id(), owner(), draft(b"bad")).await,
        Err(ArtifactStoreError::UnsupportedSchema)
    );
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_event_failure_rolls_back_metadata_but_retains_unpublished_files() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION suppress_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .create(id(), id(), owner(), draft(b"unpublished"))
        .await
        .is_err());
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    assert_eq!(f.count("collaboration_artifact_revisions").await, 0);
    assert_eq!(f.files(), 1);
    f.sql("DROP TRIGGER suppress_event ON collaboration_artifact_outbox")
        .await;
    let before = f
        .store
        .create(id(), id(), owner(), draft(b"old"))
        .await
        .unwrap();
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .revise(&before.reference, id(), owner(), draft(b"unpublished edit"))
        .await
        .is_err());
    assert_eq!(
        f.store
            .read(&before.reference, owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"old"
    );
    assert_eq!(f.count("collaboration_artifact_revisions").await, 1);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_deferred_commit_failure_never_publishes_metadata() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION fail_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_commit AFTER INSERT ON collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_commit()").await;
    assert_eq!(
        f.store
            .create(id(), id(), owner(), draft(b"unpublished"))
            .await,
        Err(ArtifactStoreError::StorageUnavailable)
    );
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 0);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_cancelled_publication_keeps_evidence_without_partial_metadata() {
    let f = Fixture::ready().await;
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .create(id(), id(), owner(), draft(b"unpublished"))
            .await
    });
    f.wait_blocked().await;
    assert_eq!(f.files(), 1);
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    // Acquire an exclusive table lock to wait for the cancelled transaction's rollback.
    let mut tx = f.pool.begin().await.unwrap();
    query("LOCK TABLE collaboration_artifacts IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 0);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_reject_missing_changed_and_oversized_content() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"original"))
        .await
        .unwrap();
    let blob = f.blob();
    for bytes in [b"modified".to_vec(), vec![0; 1_048_577]] {
        fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&blob, bytes).unwrap();
        fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(f.store.read(&first.reference, owner()).await.is_err());
        assert!(f
            .store
            .revise(&first.reference, id(), owner(), draft(b"bad"))
            .await
            .is_err());
    }
    fs::remove_file(&blob).unwrap();
    assert!(f.store.read(&first.reference, owner()).await.is_err());
    assert_eq!(f.count("collaboration_artifact_revisions").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_reject_symbolic_hard_links_and_special_files() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"original"))
        .await
        .unwrap();
    let blob = f.blob();
    let saved = f.directory.join("saved");
    fs::rename(&blob, &saved).unwrap();
    symlink(&saved, &blob).unwrap();
    assert!(f.store.read(&first.reference, owner()).await.is_err());
    fs::remove_file(&blob).unwrap();
    fs::hard_link(&saved, &blob).unwrap();
    assert!(f.store.read(&first.reference, owner()).await.is_err());
    fs::remove_file(&blob).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(blob.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a live NUL-terminated path in this test's private directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(1),
        f.store.read(&first.reference, owner())
    )
    .await
    .unwrap()
    .is_err());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_replaced_blob_root_cannot_acknowledge_publication() {
    let f = Fixture::ready().await;
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .create(id(), id(), owner(), draft(b"unpublished"))
            .await
    });
    f.wait_blocked().await;
    let moved = f.directory.with_extension("moved");
    fs::rename(&f.directory, &moved).unwrap();
    fs::create_dir(&f.directory).unwrap();
    fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o700)).unwrap();
    blocker.rollback().await.unwrap();
    assert!(pending.await.unwrap().is_err());
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 1);
    fs::remove_dir(&f.directory).unwrap();
    fs::rename(moved, &f.directory).unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_readers_see_committed_revisions_during_publication() {
    let f = Fixture::ready().await;
    let before = f
        .store
        .create(id(), id(), owner(), draft(b"old"))
        .await
        .unwrap();
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let expected = before.reference.clone();
    let pending =
        tokio::spawn(async move { store.revise(&expected, id(), owner(), draft(b"new")).await });
    f.wait_blocked().await;
    assert_eq!(
        f.store
            .read(&before.reference, owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"old"
    );
    blocker.rollback().await.unwrap();
    let after = pending.await.unwrap().unwrap();
    assert_eq!(
        f.store
            .read(&after.reference, owner())
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"new"
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_post_write_scope_and_event_tampering_roll_back() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION alter_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.snapshot = '{}'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER alter_event BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION alter_event()").await;
    assert!(f
        .store
        .create(id(), id(), owner(), draft(b"unpublished"))
        .await
        .is_err());
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    f.sql("DROP TRIGGER alter_event ON collaboration_artifact_outbox")
        .await;
    f.sql("CREATE FUNCTION change_scope() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        UPDATE collaboration_artifact_schema SET workspace_id = '267c6b0f-182c-4980-a925-cdc967327c57'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER change_scope BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION change_scope()").await;
    assert!(f
        .store
        .create(id(), id(), owner(), draft(b"unpublished"))
        .await
        .is_err());
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    f.sql("DROP TRIGGER change_scope ON collaboration_artifact_outbox")
        .await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"published"))
        .await
        .unwrap();
    f.sql("UPDATE collaboration_artifact_revisions SET snapshot = jsonb_set(snapshot::jsonb, '{title}', '\"tampered\"')::text").await;
    assert!(f.store.read(&first.reference, owner()).await.is_err());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_reject_rls_missing_events_and_temporary_shadow_tables() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"private"))
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_artifacts ENABLE ROW LEVEL SECURITY")
        .await;
    assert_eq!(
        f.store.read(&first.reference, owner()).await,
        Err(ArtifactStoreError::UnsupportedSchema)
    );
    f.sql("ALTER TABLE collaboration_artifacts DISABLE ROW LEVEL SECURITY")
        .await;
    let pool = pool_for(
        &std::env::var("CYANREX_TEST_DATABASE_URL").unwrap(),
        f.schema.clone(),
        1,
    )
    .await;
    query("CREATE TEMP TABLE collaboration_artifacts (LIKE collaboration_artifacts INCLUDING ALL)")
        .execute(&pool)
        .await
        .unwrap();
    let shadow = ArtifactStore::open(pool.clone(), scope(), &f.directory).unwrap();
    assert_eq!(
        shadow.read(&first.reference, owner()).await,
        Err(ArtifactStoreError::UnsupportedSchema)
    );
    pool.close().await;
    f.sql("DELETE FROM collaboration_artifact_outbox").await;
    assert_eq!(
        f.store.read(&first.reference, owner()).await,
        Err(ArtifactStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_artifacts_existing_files_and_revision_ids_are_never_overwritten_or_adopted() {
    let f = Fixture::ready().await;
    let first = f
        .store
        .create(id(), id(), owner(), draft(b"first"))
        .await
        .unwrap();
    assert!(f
        .store
        .revise(
            &first.reference,
            first.reference.revision_id,
            owner(),
            draft(b"second")
        )
        .await
        .is_err());
    assert_eq!(f.files(), 1);
    // Simulate a rolled-back database publication: its bytes are retained as evidence.
    f.sql("DELETE FROM collaboration_artifact_outbox").await;
    f.sql("DELETE FROM collaboration_artifact_revisions").await;
    f.sql("DELETE FROM collaboration_artifacts").await;
    assert!(f
        .store
        .create(
            first.reference.artifact_id,
            first.reference.revision_id,
            owner(),
            draft(b"first")
        )
        .await
        .is_err());
    assert_eq!(fs::read(f.blob()).unwrap(), b"first");
    assert_eq!(f.count("collaboration_artifacts").await, 0);
    f.cleanup().await;
}
