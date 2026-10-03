use super::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;

async fn assert_empty_metadata(f: &Fixture) {
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        assert_eq!(f.count(table).await, 0, "{table}");
    }
}

fn reference(
    artifact_id: ArtifactId,
    revision_id: ArtifactRevisionId,
    bytes: &[u8],
) -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id,
        revision_id,
        sha256: format!("{:x}", Sha256::digest(bytes)).parse().unwrap(),
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_invalid_sources_scopes_and_namespaces_write_no_files() {
    let f = Fixture::ready().await;
    let wrong_source = DurableAuthSource::new(f.auth.base.pool.clone(), id());
    assert!(wrong_source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            id(),
            draft(b"Denied")
        )
        .await
        .is_err());
    for namespace in [
        f.auth.base.schema.clone(),
        format!("missing_artifacts_{}", Uuid::new_v4().simple()),
    ] {
        let workspace = f
            .auth
            .source
            .open_artifact_workspace(&namespace, scope(), &f.directory)
            .unwrap();
        assert_eq!(
            f.auth
                .source
                .create_session_artifact(
                    &f.auth.learner_token,
                    &workspace,
                    id(),
                    id(),
                    draft(b"Wrong namespace"),
                )
                .await,
            Err(SessionArtifactError::InvalidNamespace)
        );
        assert_eq!(f.files(), 0);
    }
    let foreign = f
        .auth
        .source
        .open_artifact_workspace(
            &f.schema,
            WorkspaceRef {
                workspace_id: id(),
                ..scope()
            },
            &f.directory,
        )
        .unwrap();
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &foreign,
            id(),
            id(),
            draft(b"Foreign scope"),
        )
        .await
        .is_err());
    for (change, restore) in [
        (
            "UPDATE collaboration_artifact_schema SET version = 99".to_string(),
            "UPDATE collaboration_artifact_schema SET version = 1".to_string(),
        ),
        (
            format!(
                "UPDATE collaboration_artifact_schema SET workspace_id = '{}'",
                Uuid::new_v4()
            ),
            format!(
                "UPDATE collaboration_artifact_schema SET workspace_id = '{}'",
                scope().workspace_id
            ),
        ),
        (
            format!(
                "UPDATE collaboration_artifact_schema SET authority_id = '{}'",
                Uuid::new_v4()
            ),
            format!(
                "UPDATE collaboration_artifact_schema SET authority_id = '{}'",
                authority()
            ),
        ),
    ] {
        f.sql(&change).await;
        assert!(f
            .auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                id(),
                draft(b"Invalid metadata"),
            )
            .await
            .is_err());
        assert_eq!(f.files(), 0);
        assert_empty_metadata(&f).await;
        f.sql(&restore).await;
    }
    for table in [
        "collaboration_identity_schema",
        "collaboration_access_schema",
    ] {
        f.auth.sql(&format!("UPDATE {table} SET version = 1")).await;
        assert!(f
            .auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                id(),
                draft(b"Missing audit"),
            )
            .await
            .is_err());
        assert_eq!(f.files(), 0);
        assert_empty_metadata(&f).await;
        f.auth.sql(&format!("UPDATE {table} SET version = 2")).await;
    }
    f.auth
        .sql("UPDATE collaboration_auth_source_schema SET version = 99")
        .await;
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            id(),
            draft(b"Unsupported source"),
        )
        .await
        .is_err());
    assert_eq!(f.files(), 0);
    assert_empty_metadata(&f).await;
    f.auth
        .sql("UPDATE collaboration_auth_source_schema SET version = 1")
        .await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_event_and_commit_failures_retain_unpublished_files() {
    let f = Fixture::ready().await;
    f.sql("CREATE SEQUENCE artifact_fault_calls").await;
    f.sql(&format!(
        "CREATE FUNCTION suppress_artifact_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.artifact_fault_calls'); RETURN NULL; END $$",
        f.schema
    ))
    .await;
    f.sql(
        "CREATE TRIGGER suppress_artifact_event BEFORE INSERT ON collaboration_artifact_outbox
        FOR EACH ROW EXECUTE FUNCTION suppress_artifact_event()",
    )
    .await;
    let unpublished = reference(id(), id(), b"Unpublished create");
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            unpublished.artifact_id,
            unpublished.revision_id,
            draft(b"Unpublished create"),
        )
        .await
        .is_err());
    assert_empty_metadata(&f).await;
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &unpublished,)
            .await
            .unwrap(),
        None
    );
    let called: bool = query("SELECT is_called FROM artifact_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("is_called");
    assert!(called, "the intended outbox suppression must have executed");
    f.sql("DROP TRIGGER suppress_artifact_event ON collaboration_artifact_outbox")
        .await;
    let before = f
        .create(&f.auth.learner_token, b"Committed old content")
        .await;
    f.sql(&format!("CREATE FUNCTION fail_artifact_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.artifact_fault_calls'); RAISE EXCEPTION 'synthetic commit failure'; END $$", f.schema)).await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_artifact_commit AFTER INSERT ON collaboration_artifact_outbox
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_artifact_commit()").await;
    let failed = reference(before.reference.artifact_id, id(), b"Unpublished revision");
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &before.reference,
                failed.revision_id,
                draft(b"Unpublished revision"),
            )
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable,)
        ))
    );
    let calls: i64 = query("SELECT last_value FROM artifact_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("last_value");
    assert_eq!(
        calls, 2,
        "deferred failure must occur at the commit boundary"
    );
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        assert_eq!(f.count(table).await, 1);
    }
    assert_eq!(f.files(), 3);
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &failed,)
            .await
            .unwrap(),
        None
    );
    let old = f
        .auth
        .source
        .read_session_artifact(&f.auth.learner_token, &f.workspace, &before.reference)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.revision, before);
    assert_eq!(old.bytes, b"Committed old content");
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_post_write_authority_changes_roll_back_but_keep_files() {
    let f = Fixture::ready().await;
    let actor = f.auth.learner.principal.reference.principal_id;
    let source = &f.auth.base.schema;
    f.sql("CREATE SEQUENCE artifact_authority_fault_calls")
        .await;
    for (index, body) in [
        format!("DELETE FROM {source}.sessions"),
        format!("UPDATE {source}.collaboration_memberships SET status = 'suspended' WHERE principal_id = '{actor}'"),
        format!("UPDATE {source}.collaboration_principals SET status = 'disabled' WHERE principal_id = '{actor}';
            UPDATE {source}.collaboration_legacy_identities SET retired_at = clock_timestamp() WHERE principal_id = '{actor}'"),
        format!("UPDATE {source}.collaboration_workspaces SET status = 'archived'"),
        format!("UPDATE {source}.collaboration_auth_source_schema SET authority_id = '{}'", Uuid::new_v4()),
    ].into_iter().enumerate() {
        f.sql(&format!("CREATE OR REPLACE FUNCTION change_artifact_authority() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.artifact_authority_fault_calls'); {body}; RETURN NEW; END $$", f.schema)).await;
        f.sql("CREATE TRIGGER change_artifact_authority BEFORE INSERT ON collaboration_artifact_outbox
            FOR EACH ROW EXECUTE FUNCTION change_artifact_authority()").await;
        let unpublished = reference(id(), id(), b"Must not publish");
        assert!(f.auth.source.create_session_artifact(
            &f.auth.learner_token, &f.workspace, unpublished.artifact_id, unpublished.revision_id,
            draft(b"Must not publish"),
        ).await.is_err());
        let calls: i64 = query("SELECT last_value FROM artifact_authority_fault_calls")
            .fetch_one(&f.pool).await.unwrap().get("last_value");
        assert_eq!(calls, index as i64 + 1, "post-write authority fault must execute");
        assert_empty_metadata(&f).await;
        assert_eq!(f.files(), index + 1);
        f.sql("DROP TRIGGER change_artifact_authority ON collaboration_artifact_outbox").await;
        assert!(f.auth.source.validate_session(&f.auth.learner_token).await.unwrap().is_some());
        assert_eq!(f.auth.source.read_session_artifact(
            &f.auth.learner_token, &f.workspace, &unpublished,
        ).await.unwrap(), None);
    }
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_source_rls_and_temporary_shadows_write_no_files() {
    let f = Fixture::ready().await;
    f.auth
        .sql("ALTER TABLE sessions ENABLE ROW LEVEL SECURITY")
        .await;
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            id(),
            draft(b"RLS source"),
        )
        .await
        .is_err());
    assert_empty_metadata(&f).await;
    assert_eq!(f.files(), 0);
    f.auth
        .sql("ALTER TABLE sessions DISABLE ROW LEVEL SECURITY")
        .await;
    let schema = f.auth.base.schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false)")
                    .bind(schema)
                    .execute(&mut *connection)
                    .await?;
                query("CREATE TEMP TABLE sessions (LIKE sessions INCLUDING ALL)")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let source = DurableAuthSource::new(pool.clone(), authority());
    let workspace = source
        .open_artifact_workspace(&f.schema, scope(), &f.directory)
        .unwrap();
    assert_eq!(
        source
            .create_session_artifact(
                &f.auth.learner_token,
                &workspace,
                id(),
                id(),
                draft(b"Shadow source"),
            )
            .await,
        Err(SessionArtifactError::InvalidNamespace)
    );
    pool.close().await;
    assert_empty_metadata(&f).await;
    assert_eq!(f.files(), 0);
    drop(workspace);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_valid_clone_namespace_switch_cannot_publish() {
    let f = Fixture::ready().await;
    let clone_schema = format!("{}_clone", f.schema);
    query(&format!("CREATE SCHEMA {clone_schema}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone_schema).await;
    let clone_store = ArtifactStore::open(clone_pool.clone(), scope(), &f.directory).unwrap();
    clone_store.install_empty_namespace().await.unwrap();
    f.sql("CREATE SEQUENCE artifact_path_fault_calls").await;
    f.sql(&format!("CREATE FUNCTION change_artifact_path() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.artifact_path_fault_calls');
        INSERT INTO {clone_schema}.collaboration_artifacts SELECT * FROM {}.collaboration_artifacts WHERE artifact_id = NEW.artifact_id;
        INSERT INTO {clone_schema}.collaboration_artifact_revisions SELECT * FROM {}.collaboration_artifact_revisions WHERE revision_id = NEW.revision_id;
        INSERT INTO {clone_schema}.collaboration_artifact_outbox
            (event_id, artifact_id, revision_id, sequence, actor_id, event_type, snapshot)
            VALUES (NEW.event_id, NEW.artifact_id, NEW.revision_id, NEW.sequence, NEW.actor_id, NEW.event_type, NEW.snapshot);
        PERFORM set_config('search_path', '{clone_schema}', true); RETURN NEW; END $$", f.schema, f.schema, f.schema)).await;
    f.sql(
        "CREATE TRIGGER change_artifact_path BEFORE INSERT ON collaboration_artifact_outbox
        FOR EACH ROW EXECUTE FUNCTION change_artifact_path()",
    )
    .await;
    // Both stores share the pinned root; the cloned rows and bytes permit inner Artifact
    // readback. Only the outer namespace pin must reject this otherwise coherent publication.
    let unpublished = reference(id(), id(), b"Same bytes in clone");
    assert_eq!(
        f.auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                unpublished.artifact_id,
                unpublished.revision_id,
                draft(b"Same bytes in clone"),
            )
            .await,
        Err(SessionArtifactError::InvalidNamespace)
    );
    let called: bool = query("SELECT is_called FROM artifact_path_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("is_called");
    assert!(called);
    assert_empty_metadata(&f).await;
    assert_eq!(f.files(), 1);
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        let count: i64 = query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&clone_pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(count, 0);
    }
    assert_eq!(
        clone_store
            .read(&unpublished, f.auth.learner.principal.reference)
            .await
            .unwrap(),
        None
    );
    f.assert_pool_restored().await;
    clone_pool.close().await;
    drop(clone_store);
    query(&format!("DROP SCHEMA {clone_schema} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_replaced_roots_and_corrupt_bytes_never_escape() {
    let f = Fixture::ready().await;
    let first = f
        .create(&f.auth.learner_token, b"Original immutable content")
        .await;
    let filename = f.blob().file_name().unwrap().to_owned();
    let moved = f.directory.with_extension("moved");
    fs::rename(&f.directory, &moved).unwrap();
    fs::create_dir(&f.directory).unwrap();
    fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference,)
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &first.reference,
                id(),
                draft(b"Rejected replacement"),
            )
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.files(), 0);
    assert_eq!(fs::read_dir(&moved).unwrap().count(), 1);
    assert_eq!(
        fs::read(moved.join(&filename)).unwrap(),
        b"Original immutable content"
    );
    fs::remove_dir(&f.directory).unwrap();
    fs::rename(&moved, &f.directory).unwrap();
    let blob = f.blob();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    let mut corrupt = b"Original immutable content".to_vec();
    corrupt[0] ^= 1;
    fs::write(&blob, &corrupt).unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference,)
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &first.reference,
                id(),
                draft(b"Rejected corrupt parent"),
            )
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.files(), 1);
    assert_eq!(fs::read(&blob).unwrap(), corrupt);
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        assert_eq!(f.count(table).await, 1);
    }
    f.assert_pool_restored().await;
    f.cleanup().await;
}
