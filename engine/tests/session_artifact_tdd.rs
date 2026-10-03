#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactError,
            SessionArtifactWorkspace, SessionBindCommand, SessionCommandError,
            SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
#[path = "session_artifact/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_artifact/concurrency.rs"]
mod concurrency;
#[path = "session_artifact/faults.rs"]
mod faults;
#[path = "session_artifact/lifecycle.rs"]
mod lifecycle;

#[tokio::test]
async fn session_artifact_configuration_is_explicit_and_no_failure_writes_a_blob() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    let directory = private_directory();
    for name in ["", "public", "pg_catalog", "a,b", "x\"", "a.b", "Bad"] {
        assert!(matches!(
            source.open_artifact_workspace(name, scope(), &directory),
            Err(SessionArtifactError::InvalidNamespace)
        ));
    }
    let workspace = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    assert_eq!(
        source
            .create_session_artifact("invalid", &workspace, id(), id(), draft(b"private"))
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(
        source
            .create_session_artifact(
                &Uuid::new_v4().to_string(),
                &workspace,
                id(),
                id(),
                draft(b"private")
            )
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        ))
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_artifact_configuration_never_creates_or_repairs_a_private_root() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    let root = private_directory();
    let missing = root.join("missing");
    assert!(matches!(
        source.open_artifact_workspace("private_artifacts", scope(), &missing),
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    ));
    assert!(!missing.exists());
    let link = root.join("link");
    symlink(&root, &link).unwrap();
    assert!(source
        .open_artifact_workspace("private_artifacts", scope(), &link)
        .is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(source
        .open_artifact_workspace("private_artifacts", scope(), &root)
        .is_err());
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o755
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn session_artifact_authorization_stays_inside_source_transaction_without_live_wiring() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter = fs::read_to_string(
        root.join("src/services/auth_service/durable_source/artifact_commands.rs"),
    )
    .unwrap();
    let guard = fs::read_to_string(
        root.join("src/services/auth_service/durable_source/private_work/mod.rs"),
    )
    .unwrap();
    for source in [&adapter, &guard] {
        for forbidden in [
            "validate_session(",
            "command_actor(",
            "LegacyRole",
            "std::env",
            "AppState",
        ] {
            assert!(!source.contains(forbidden), "{forbidden}");
        }
    }
    for helper in [
        "create_in_transaction",
        "revise_in_transaction",
        "read_in_transaction",
        "begin_private_work",
        "context.finish",
    ] {
        assert!(adapter.contains(helper), "{helper}");
    }
    assert!(guard.contains("recheck_command_session"));
    for file in ["src/application.rs", "src/state.rs"] {
        assert!(!fs::read_to_string(root.join(file))
            .unwrap()
            .contains("SessionArtifactWorkspace"));
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_derive_owner_and_preserve_exact_old_content() {
    let f = Fixture::ready().await;
    let first = f
        .create(&f.auth.learner_token, b"First private draft")
        .await;
    assert_eq!(first.owner, f.auth.learner.principal.reference);
    let next = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &first.reference,
            id(),
            draft(b"Second private draft"),
        )
        .await
        .unwrap();
    assert_eq!(next.parent, Some(first.reference.clone()));
    for (revision, bytes) in [
        (&first, b"First private draft".as_slice()),
        (&next, b"Second private draft".as_slice()),
    ] {
        let content = f
            .auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &revision.reference)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&content.revision, revision);
        assert_eq!(content.bytes, bytes);
    }
    assert_eq!(f.count("collaboration_artifact_outbox").await, 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_managers_cannot_touch_private_content_even_when_blob_is_missing(
) {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token, b"Private content").await;
    fs::remove_file(f.blob()).unwrap();
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.manager_token, &f.workspace, &first.reference)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.manager_token,
                &f.workspace,
                &first.reference,
                id(),
                draft(b"Not permitted")
            )
            .await,
        Err(SessionArtifactError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(f.files(), 0);
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference)
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.count("collaboration_artifact_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_revision_races_and_duplicate_ids_do_not_write_extra_files() {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token, b"First").await;
    let (a, b) = tokio::join!(
        f.auth.source.revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &first.reference,
            id(),
            draft(b"Winner A")
        ),
        f.auth.source.revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &first.reference,
            id(),
            draft(b"Winner B")
        )
    );
    let next = if let Ok(winner) = a {
        assert_eq!(
            b,
            Err(SessionArtifactError::Artifact(
                ArtifactStoreError::StaleRevision
            ))
        );
        winner
    } else {
        assert_eq!(
            a,
            Err(SessionArtifactError::Artifact(
                ArtifactStoreError::StaleRevision
            ))
        );
        b.unwrap()
    };
    assert_eq!(f.files(), 2);
    for (artifact, revision) in [
        (first.reference.artifact_id, id()),
        (id(), first.reference.revision_id),
    ] {
        assert_eq!(
            f.auth
                .source
                .create_session_artifact(
                    &f.auth.learner_token,
                    &f.workspace,
                    artifact,
                    revision,
                    draft(b"Duplicate")
                )
                .await,
            Err(SessionArtifactError::Artifact(ArtifactStoreError::Conflict))
        );
    }
    let retyped = ArtifactDraft::new(
        "sample.other".parse().unwrap(),
        "Retyped",
        "text/plain",
        b"Different".to_vec(),
    )
    .unwrap();
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &next.reference,
                id(),
                retyped
            )
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::InvalidInput
        ))
    );
    assert_eq!(f.files(), 2);
    assert_eq!(f.count("collaboration_artifacts").await, 1);
    assert_eq!(f.count("collaboration_artifact_revisions").await, 2);
    let events = query("SELECT snapshot FROM collaboration_artifact_outbox")
        .fetch_all(&f.pool)
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
    for event in events {
        let raw: String = event.get("snapshot");
        assert!(!raw.contains(&f.auth.learner_token));
        assert!(!raw.contains(&source_fixture::token_hash(&f.auth.learner_token)));
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_exact_references_never_grant_cross_scope_or_digest_access() {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token, b"Same bytes").await;
    let other = f.create(&f.auth.manager_token, b"Same bytes").await;
    assert_eq!(first.reference.sha256, other.reference.sha256);
    assert_eq!(f.files(), 2);
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &other.reference)
            .await
            .unwrap(),
        None
    );
    let wrong_digest = ArtifactRef {
        sha256: "f".repeat(64).parse().unwrap(),
        ..first.reference.clone()
    };
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &wrong_digest)
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::InvalidRecord
        ))
    );
    let missing = ArtifactRef {
        revision_id: id(),
        ..first.reference.clone()
    };
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &missing)
            .await
            .unwrap(),
        None
    );
    let foreign = ArtifactRef {
        workspace: WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        ..first.reference.clone()
    };
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &foreign)
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::ScopeMismatch
        ))
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &foreign,
                id(),
                draft(b"No")
            )
            .await,
        Err(SessionArtifactError::Artifact(
            ArtifactStoreError::ScopeMismatch
        ))
    );
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}
