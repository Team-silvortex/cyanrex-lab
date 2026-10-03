use super::*;
use std::os::unix::fs::PermissionsExt;

async fn calls(f: &Fixture) -> i64 {
    let row = query("SELECT last_value, is_called FROM review_fault_calls")
        .fetch_one(&f.review_pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "fault trigger was never reached"
    );
    row.get("last_value")
}

async fn unchanged_review(f: &Fixture, before: Option<&ReviewRecord>) {
    for table in [
        "collaboration_reviews",
        "collaboration_review_revisions",
        "collaboration_review_outbox",
    ] {
        assert_eq!(f.count_review(table).await, i64::from(before.is_some()));
    }
    if let Some(before) = before {
        assert_eq!(
            f.reviews
                .read(before.reference, before.reviewer)
                .await
                .unwrap(),
            Some(before.clone())
        );
    }
}

async fn unchanged_artifact(f: &Fixture, revision: &ArtifactRevision, bytes: &[u8]) {
    let content = f
        .artifacts
        .read(&revision.reference, revision.owner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&content.revision, revision);
    assert_eq!(content.bytes, bytes);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_post_write_evidence_and_session_faults_roll_back_mutations() {
    for revise in [false, true] {
        let f = Fixture::ready().await;
        let target = f
            .create_artifact(&f.auth.learner_token, b"Valid target")
            .await;
        let evidence = f
            .create_artifact(&f.auth.learner_token, b"Separate supporting evidence")
            .await;
        let before = if revise {
            Some(
                f.create(
                    &f.auth.learner_token,
                    vec![target.reference.clone()],
                    vec![evidence.reference.clone()],
                )
                .await,
            )
        } else {
            None
        };
        let session = f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .unwrap();
        f.sql_review("CREATE SEQUENCE review_fault_calls").await;
        for (index, (session_fault, body)) in [
            (false, format!("UPDATE {}.collaboration_artifact_schema SET workspace_id = '{}'", f.artifact_schema, Uuid::new_v4())),
            (false, format!("UPDATE {}.collaboration_artifacts SET owner_id = '{}' WHERE artifact_id = '{}'", f.artifact_schema, f.auth.manager.principal.reference.principal_id, evidence.reference.artifact_id)),
            (false, format!("DELETE FROM {}.collaboration_artifact_outbox WHERE artifact_id = '{}'", f.artifact_schema, evidence.reference.artifact_id)),
            (true, format!("DELETE FROM {}.sessions WHERE token = '{}'", f.auth.base.schema, source_fixture::token_hash(&f.auth.learner_token))),
        ].into_iter().enumerate() {
            f.sql_review(&format!("CREATE OR REPLACE FUNCTION corrupt_review_context() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM nextval('{}.review_fault_calls'); {body}; RETURN NEW; END $$", f.review_schema)).await;
            f.sql_review("CREATE TRIGGER corrupt_review_context BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION corrupt_review_context()").await;
            let result = match &before {
                Some(record) => f.revise(&f.auth.learner_token, record.reference, edit(HumanReviewVerdict::Commented, "Do not publish this amendment")).await,
                None => f.auth.source.create_session_review(&f.auth.learner_token, &f.workspace, id(), vec![target.reference.clone()], vec![evidence.reference.clone()], edit(HumanReviewVerdict::Approved, "")).await,
            };
            if session_fault {
                assert_eq!(result, Err(SessionReviewError::Session(SessionCommandError::InvalidSession)));
            } else {
                assert!(matches!(result, Err(SessionReviewError::Artifact(_))), "{result:?}");
            }
            assert_eq!(calls(&f).await, index as i64 + 1, "must reach the Review event after initial content validation");
            unchanged_review(&f, before.as_ref()).await;
            assert_eq!(f.auth.source.validate_session(&f.auth.learner_token).await.unwrap(), Some(session.clone()));
            for table in ["collaboration_artifacts", "collaboration_artifact_revisions", "collaboration_artifact_outbox"] {
                assert_eq!(f.count_artifact(table).await, 2);
            }
            assert_eq!(f.files(), 2);
            unchanged_artifact(&f, &target, b"Valid target").await;
            unchanged_artifact(&f, &evidence, b"Separate supporting evidence").await;
            f.sql_review("DROP TRIGGER corrupt_review_context ON collaboration_review_outbox").await;
        }
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_artifact_replacement_preserves_original_namespace_pin() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Pinned before Review write")
        .await;
    let clone = format!("review_artifact_clone_{}", Uuid::new_v4().simple());
    let moved = format!("review_artifact_moved_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone).await;
    let clone_store = ArtifactStore::open(clone_pool.clone(), scope(), &f.directory).unwrap();
    clone_store.install_empty_namespace().await.unwrap();
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        query(&format!(
            "INSERT INTO {clone}.{table} SELECT * FROM {}.{table}",
            f.artifact_schema
        ))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    }
    // Both namespaces contain identical valid content metadata and use the same private root.
    // Only the first-observed namespace OID distinguishes the configured store from its clone.
    f.sql_review("CREATE SEQUENCE review_fault_calls").await;
    f.sql_review(&format!(
        "CREATE FUNCTION replace_review_artifacts() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.review_fault_calls'); ALTER SCHEMA {} RENAME TO {moved};
        ALTER SCHEMA {clone} RENAME TO {}; RETURN NEW; END $$",
        f.review_schema, f.artifact_schema, f.artifact_schema
    ))
    .await;
    f.sql_review("CREATE TRIGGER replace_review_artifacts BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION replace_review_artifacts()").await;
    assert_eq!(
        f.auth
            .source
            .create_session_review(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                vec![artifact.reference.clone()],
                vec![],
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(SessionReviewError::InvalidNamespace)
    );
    assert_eq!(calls(&f).await, 1);
    unchanged_review(&f, None).await;
    assert_eq!(f.files(), 1);
    unchanged_artifact(&f, &artifact, b"Pinned before Review write").await;
    assert_eq!(
        clone_store
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .revision,
        artifact
    );
    let moved_exists: bool =
        query("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1) AS present")
            .bind(&moved)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("present");
    assert!(
        !moved_exists,
        "namespace renames must roll back with the Review"
    );
    f.assert_pool_restored().await;
    clone_pool.close().await;
    drop(clone_store);
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_valid_review_clone_path_cannot_escape_namespace_checks() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Legitimate review subject")
        .await;
    let clone = format!("review_clone_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone).await;
    ReviewStore::new(clone_pool.clone(), scope())
        .install_empty_namespace()
        .await
        .unwrap();
    f.sql_review("CREATE SEQUENCE review_fault_calls").await;
    f.sql_review(&format!("CREATE FUNCTION switch_review_path() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.review_fault_calls');
        INSERT INTO {clone}.collaboration_reviews SELECT * FROM {}.collaboration_reviews WHERE review_id = NEW.review_id;
        INSERT INTO {clone}.collaboration_review_revisions SELECT * FROM {}.collaboration_review_revisions WHERE review_id = NEW.review_id;
        INSERT INTO {clone}.collaboration_review_outbox (event_id, review_id, revision, actor_id, event_type, snapshot)
            VALUES (NEW.event_id, NEW.review_id, NEW.revision, NEW.actor_id, NEW.event_type, NEW.snapshot);
        PERFORM set_config('search_path', '{clone}', true); RETURN NEW; END $$", f.review_schema, f.review_schema, f.review_schema)).await;
    f.sql_review("CREATE TRIGGER switch_review_path BEFORE INSERT ON collaboration_review_outbox FOR EACH ROW EXECUTE FUNCTION switch_review_path()").await;
    assert_eq!(
        f.auth
            .source
            .create_session_review(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                vec![artifact.reference.clone()],
                vec![],
                edit(HumanReviewVerdict::Approved, "")
            )
            .await,
        Err(SessionReviewError::InvalidNamespace)
    );
    assert_eq!(calls(&f).await, 1);
    unchanged_review(&f, None).await;
    for table in [
        "collaboration_reviews",
        "collaboration_review_revisions",
        "collaboration_review_outbox",
    ] {
        let count: i64 = query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&clone_pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(count, 0);
    }
    assert_eq!(f.files(), 1);
    unchanged_artifact(&f, &artifact, b"Legitimate review subject").await;
    f.assert_pool_restored().await;
    clone_pool.close().await;
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_suppressed_rows_and_deferred_commit_leave_inputs_unchanged() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Existing immutable subject")
        .await;
    f.sql_review("CREATE SEQUENCE review_fault_calls").await;
    f.sql_review(&format!(
        "CREATE FUNCTION suppress_review_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.review_fault_calls'); RETURN NULL; END $$",
        f.review_schema
    ))
    .await;
    for (index, table) in [
        "collaboration_reviews",
        "collaboration_review_revisions",
        "collaboration_review_outbox",
    ]
    .into_iter()
    .enumerate()
    {
        f.sql_review(&format!("CREATE TRIGGER suppress_review_write BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION suppress_review_write()")).await;
        assert!(f
            .auth
            .source
            .create_session_review(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                vec![artifact.reference.clone()],
                vec![],
                edit(HumanReviewVerdict::Approved, "")
            )
            .await
            .is_err());
        assert_eq!(calls(&f).await, index as i64 + 1);
        unchanged_review(&f, None).await;
        assert_eq!(f.files(), 1);
        f.sql_review(&format!("DROP TRIGGER suppress_review_write ON {table}"))
            .await;
    }
    let before = f
        .create(
            &f.auth.learner_token,
            vec![artifact.reference.clone()],
            vec![],
        )
        .await;
    f.sql_review(&format!("CREATE FUNCTION fail_review_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.review_fault_calls'); RAISE EXCEPTION 'synthetic Review commit failure'; END $$", f.review_schema)).await;
    f.sql_review("CREATE CONSTRAINT TRIGGER fail_review_commit AFTER INSERT ON collaboration_review_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_review_commit()").await;
    assert_eq!(
        f.revise(
            &f.auth.learner_token,
            before.reference,
            edit(HumanReviewVerdict::Commented, "Uncommitted opinion")
        )
        .await,
        Err(SessionReviewError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        ))
    );
    assert_eq!(
        calls(&f).await,
        4,
        "must reach deferred failure after final admission"
    );
    unchanged_review(&f, Some(&before)).await;
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    unchanged_artifact(&f, &artifact, b"Existing immutable subject").await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_reviews_corrupt_content_blocks_read_and_revise_without_repair() {
    let f = Fixture::ready().await;
    let target = f
        .create_artifact(&f.auth.learner_token, b"Valid target")
        .await;
    let evidence = f
        .create_artifact(&f.auth.learner_token, b"Evidence must also be checked")
        .await;
    let before = f
        .create(
            &f.auth.learner_token,
            vec![target.reference.clone()],
            vec![evidence.reference.clone()],
        )
        .await;
    let blob = f.blob(&evidence.reference);
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    let mut changed = b"Evidence must also be checked".to_vec();
    changed[0] ^= 1;
    fs::write(&blob, &changed).unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert_eq!(
        f.read(&f.auth.learner_token, before.reference).await,
        Err(SessionReviewError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(
        f.revise(
            &f.auth.learner_token,
            before.reference,
            edit(
                HumanReviewVerdict::Commented,
                "Cannot ignore damaged evidence"
            )
        )
        .await,
        Err(SessionReviewError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    unchanged_review(&f, Some(&before)).await;
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    assert_eq!(fs::read(blob).unwrap(), changed);
    unchanged_artifact(&f, &target, b"Valid target").await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}
