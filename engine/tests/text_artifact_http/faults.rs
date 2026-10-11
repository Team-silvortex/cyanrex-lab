use super::behavior::{counts, failure, publish, reference};
use super::concurrency::{backend, calls, pause_event, wait_for, Pending};
use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_current_session_and_membership_gate_every_read_and_create() {
    for mode in ["logout", "expired", "suspended"] {
        let f = ContentFixture::ready().await;
        let saved = publish(&f, "Never disclosed after denial").await;
        for reading in [false, true] {
            let req = if reading {
                request(
                    "GET",
                    &text_path(&saved.reference),
                    &f.auth.target_token,
                    Body::empty(),
                )
            } else {
                request(
                    "POST",
                    TEXT_COLLECTION,
                    &f.auth.target_token,
                    create_body(id(), id(), "Denied", "private"),
                )
            };
            error(
                app(&f),
                req,
                StatusCode::FORBIDDEN,
                "access_denied",
                "unconfirmed",
            )
            .await;
        }
        match mode {
            "logout" => f.auth.source.logout(&f.auth.learner_token).await.unwrap(),
            "expired" => {
                query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
            }
            _ => {
                let current = f.auth.current(f.auth.learner.binding.principal_id).await;
                let mut desired = current.policy;
                desired.membership.status = MembershipStatus::Suspended;
                f.auth
                    .source
                    .apply_session_policy_command(
                        &f.auth.manager_token,
                        &SessionPolicyCommand {
                            command_id: id(),
                            expected_revision: Some(current.revision),
                            desired,
                        },
                    )
                    .await
                    .unwrap();
            }
        }
        let (status, code) = if mode == "suspended" {
            (StatusCode::FORBIDDEN, "access_denied")
        } else {
            (StatusCode::UNAUTHORIZED, "invalid_session")
        };
        for req in [
            request(
                "GET",
                &text_path(&saved.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(id(), id(), "Denied", "private"),
            ),
        ] {
            error(app(&f), req, status, code, "unconfirmed").await;
        }
        counts(&f, 1).await;
        assert_eq!(f.files(), 1);
        assert_eq!(
            fs::read(f.blob(&saved.reference)).unwrap(),
            b"Never disclosed after denial"
        );
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_expiry_after_exact_storage_wait_never_confirms_or_discloses() {
    for reading in [false, true] {
        let f = ContentFixture::ready().await;
        let saved = publish(&f, "Original private text").await;
        let mut holder = if reading {
            let mut tx = f.artifact_pool.begin().await.unwrap();
            query(
                "SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id=$1 FOR UPDATE",
            )
            .bind(saved.reference.artifact_id.as_uuid())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            tx
        } else {
            pause_event(&f).await
        };
        let blocker = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '800 milliseconds' WHERE token=$1")
            .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let attempted = reference(id(), id(), b"Expired publication");
        let req = if reading {
            request(
                "GET",
                &text_path(&saved.reference),
                &f.auth.learner_token,
                Body::empty(),
            )
        } else {
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(
                    attempted.artifact_id,
                    attempted.revision_id,
                    "Expired title",
                    "Expired publication",
                ),
            )
        };
        let pending = Pending::start(app(&f), req);
        wait_for(&f, blocker).await;
        if !reading {
            assert_eq!(calls(&f).await, 1);
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let expired: bool = query(&format!("SELECT clock_timestamp()>=expires_at AS expired FROM {}.sessions WHERE token=$1", f.auth.base.schema))
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
                if expired { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        failure(
            pending.response().await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
        )
        .await;
        f.assert_pool_restored().await;
        counts(&f, 1).await;
        assert_eq!(
            fs::read(f.blob(&saved.reference)).unwrap(),
            b"Original private text"
        );
        assert_eq!(f.files(), if reading { 1 } else { 2 });
        if !reading {
            assert_eq!(
                fs::read(f.blob(&attempted)).unwrap(),
                b"Expired publication"
            );
        }
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_outbox_and_commit_faults_preserve_orphans_without_adoption() {
    for deferred in [false, true] {
        let f = ContentFixture::ready().await;
        let old = publish(&f, "Committed original").await;
        f.sql_artifact("CREATE SEQUENCE text_http_calls").await;
        let action = if deferred {
            "RAISE EXCEPTION 'synthetic private commit detail';"
        } else {
            "RETURN NULL;"
        };
        f.sql_artifact(&format!(
            "CREATE FUNCTION text_http_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.text_http_calls'); {action} RETURN NEW; END $$",
            f.artifact_schema
        ))
        .await;
        f.sql_artifact(if deferred {
            "CREATE CONSTRAINT TRIGGER text_http_fault AFTER INSERT ON collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION text_http_fault()"
        } else {
            "CREATE TRIGGER text_http_fault BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION text_http_fault()"
        }).await;
        let attempted = reference(id(), id(), b"Unconfirmed content");
        error(
            app(&f),
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(
                    attempted.artifact_id,
                    attempted.revision_id,
                    "Unconfirmed title",
                    "Unconfirmed content",
                ),
            ),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
        assert_eq!(
            calls(&f).await,
            1,
            "the selected write/commit fault must execute"
        );
        f.assert_pool_restored().await;
        counts(&f, 1).await;
        assert_eq!(f.files(), 2);
        assert_eq!(
            fs::read(f.blob(&attempted)).unwrap(),
            b"Unconfirmed content"
        );
        error(
            app(&f),
            request(
                "GET",
                &text_path(&attempted),
                &f.auth.learner_token,
                Body::empty(),
            ),
            StatusCode::NOT_FOUND,
            "not_found",
            "unconfirmed",
        )
        .await;
        f.sql_artifact("DROP TRIGGER text_http_fault ON collaboration_artifact_outbox")
            .await;
        // The HTTP client cannot adopt a retained file by replaying exactly the same IDs and bytes.
        error(
            app(&f),
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(
                    attempted.artifact_id,
                    attempted.revision_id,
                    "Unconfirmed title",
                    "Unconfirmed content",
                ),
            ),
            StatusCode::CONFLICT,
            "conflict",
            "unconfirmed",
        )
        .await;
        counts(&f, 1).await;
        assert_eq!(f.files(), 2);
        assert_eq!(
            fs::read(f.blob(&attempted)).unwrap(),
            b"Unconfirmed content"
        );
        let read = response(
            app(&f),
            request(
                "GET",
                &text_path(&old.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            StatusCode::OK,
        )
        .await;
        assert_eq!(read["artifact"], json!(old));
        assert_eq!(read["text"], "Committed original");
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_schema_root_and_blob_failures_never_return_text_or_repair() {
    let f = ContentFixture::ready().await;
    let saved = publish(&f, "Original immutable text").await;
    let route = app(&f);
    for mode in [
        "version",
        "scope",
        "root_mode",
        "root_replaced",
        "blob_mode",
        "blob_bytes",
        "blob_missing",
    ] {
        let blob = f.blob(&saved.reference);
        let moved = f.directory.with_extension("text-http-moved");
        match mode {
            "version" => {
                f.sql_artifact("UPDATE collaboration_artifact_schema SET version=99")
                    .await
            }
            "scope" => {
                f.sql_artifact(&format!(
                    "UPDATE collaboration_artifact_schema SET workspace_id='{}'",
                    Uuid::new_v4()
                ))
                .await
            }
            "root_mode" => {
                fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o755)).unwrap()
            }
            "root_replaced" => {
                fs::rename(&f.directory, &moved).unwrap();
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&f.directory)
                    .unwrap();
            }
            "blob_mode" => fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap(),
            "blob_bytes" => {
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&blob, b"Modified immutable text").unwrap();
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
            }
            _ => fs::remove_file(&blob).unwrap(),
        }
        let (status, code) = if mode == "scope" {
            (StatusCode::BAD_REQUEST, "invalid_request")
        } else {
            (StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
        };
        error(
            route.clone(),
            request(
                "GET",
                &text_path(&saved.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            status,
            code,
            "unconfirmed",
        )
        .await;
        if matches!(mode, "version" | "scope" | "root_mode" | "root_replaced") {
            error(
                route.clone(),
                request(
                    "POST",
                    TEXT_COLLECTION,
                    &f.auth.learner_token,
                    create_body(id(), id(), "Rejected", "No repair"),
                ),
                status,
                code,
                "unconfirmed",
            )
            .await;
        }
        match mode {
            "version" => {
                f.sql_artifact("UPDATE collaboration_artifact_schema SET version=1")
                    .await
            }
            "scope" => {
                f.sql_artifact(&format!(
                    "UPDATE collaboration_artifact_schema SET workspace_id='{}'",
                    scope().workspace_id
                ))
                .await
            }
            "root_mode" => {
                assert_eq!(
                    fs::metadata(&f.directory).unwrap().permissions().mode() & 0o777,
                    0o755
                );
                fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            "root_replaced" => {
                assert_eq!(f.files(), 0);
                assert_eq!(fs::read_dir(&moved).unwrap().count(), 1);
                fs::remove_dir(&f.directory).unwrap();
                fs::rename(&moved, &f.directory).unwrap();
            }
            "blob_mode" => {
                assert_eq!(
                    fs::metadata(&blob).unwrap().permissions().mode() & 0o777,
                    0o600
                );
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
            }
            "blob_bytes" => {
                assert_eq!(fs::read(&blob).unwrap(), b"Modified immutable text");
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&blob, b"Original immutable text").unwrap();
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
            }
            _ => assert!(!blob.exists()),
        }
        counts(&f, 1).await;
    }
    assert_eq!(f.files(), 0);
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}
