use super::*;

async fn denied_all(
    f: &Fixture,
    task: &TaskContentSnapshot,
    token: &str,
    status: StatusCode,
    code: &str,
) {
    let path = task_path(task.task.reference.task_id);
    for req in [
        request("GET", &path, token, Body::empty()),
        request(
            "POST",
            COLLECTION,
            token,
            create_body(id(), &manifest("Denied create", vec![])),
        ),
        request(
            "PUT",
            &format!("{path}/content"),
            token,
            replace_body(task, &manifest("Denied edit", vec![])),
        ),
        request(
            "POST",
            &format!("{path}/status"),
            token,
            json!({"schema_version":1,"expected_revision":task.task.revision,"status":"ready"})
                .to_string(),
        ),
    ] {
        error(app(f), req, status, code, "unconfirmed").await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_session_logout_expiry_and_membership_are_not_cached_authority() {
    for mode in ["logout", "expired", "suspended"] {
        let f = Fixture::ready().await;
        let task = create_http(&f, &manifest("Private", vec![])).await;
        denied_all(
            &f,
            &task,
            &f.auth.target_token,
            StatusCode::FORBIDDEN,
            "access_denied",
        )
        .await;
        match mode {
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
            }
            "expired" => {
                query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1").bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
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
        denied_all(&f, &task, &f.auth.learner_token, status, code).await;
        f.unchanged(&task, 1).await;
        assert_eq!(f.count_task("collaboration_tasks").await, 1);
        f.cleanup().await;
    }
}

type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
async fn blocked_by(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            if let Some(row) = query("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1").bind(blocker).fetch_optional(&f.auth.base.admin).await.unwrap() { return row.get("pid"); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("HTTP command did not reach the exact transaction blocker")
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_expiry_after_admission_and_storage_wait_returns_no_content_or_commit(
) {
    for reading in [false, true] {
        let (f, task, old, _) = content_fixture::pair().await;
        let mut holder = if reading {
            let mut tx = f.artifact_pool.begin().await.unwrap();
            query(
                "SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE",
            )
            .bind(old.reference.artifact_id.as_uuid())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            tx
        } else {
            f.pause_task_event().await
        };
        let blocker = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1").bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let route = app(&f);
        let path = task_path(task.task.reference.task_id);
        let req = if reading {
            request("GET", &path, &f.auth.learner_token, Body::empty())
        } else {
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&task, &manifest("Elapsed authority", vec![])),
            )
        };
        let pending = tokio::spawn(async move {
            error(
                route,
                req,
                StatusCode::UNAUTHORIZED,
                "invalid_session",
                "unconfirmed",
            )
            .await
        });
        blocked_by(&f, blocker).await;
        tokio::time::timeout(Duration::from_secs(2),async {
            loop {
                let expired: bool = query(&format!("SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions WHERE token = $1",f.auth.base.schema)).bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
                if expired { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        assert!(!pending.is_finished());
        holder.rollback().await.unwrap();
        pending.await.unwrap();
        f.unchanged(&task, 1).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_commit_and_outbox_faults_remain_unconfirmed_without_cookie_clearing(
) {
    for deferred in [false, true] {
        let (f, task, _, new) = content_fixture::pair().await;
        if deferred {
            f.sql_task("CREATE SEQUENCE content_fault_calls").await;
            f.sql_task(&format!("CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'private synthetic schema fault'; END $$",f.task_schema)).await;
            f.sql_task("CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()").await;
        } else {
            f.fault_trigger("RETURN NULL;").await;
        }
        let value = manifest("Would commit", vec![new.reference]);
        let path = task_path(task.task.reference.task_id);
        for req in [
            request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(id(), &value),
            ),
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&task, &value),
            ),
            request(
                "POST",
                &format!("{path}/status"),
                &f.auth.learner_token,
                json!({"schema_version":1,"expected_revision":1,"status":"ready"}).to_string(),
            ),
        ] {
            error(
                app(&f),
                req,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            )
            .await;
        }
        assert_eq!(f.fault_calls().await, 3);
        assert!(f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .is_some());
        f.unchanged(&task, 1).await;
        assert_eq!(f.count_task("collaboration_tasks").await, 1);
        assert_eq!(f.files(), 2);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_post_write_old_blob_loss_is_rechecked_before_http_success() {
    let (f, task, old, new) = content_fixture::pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let route = app(&f);
    let req = request(
        "PUT",
        &format!("{}/content", task_path(task.task.reference.task_id)),
        &f.auth.learner_token,
        replace_body(
            &task,
            &manifest("Remove old bytes", vec![new.reference.clone()]),
        ),
    );
    let pending = tokio::spawn(async move {
        error(
            route,
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await
    });
    blocked_by(&f, blocker).await;
    fs::remove_file(f.blob(&old.reference)).unwrap();
    holder.rollback().await.unwrap();
    pending.await.unwrap();
    f.unchanged(&task, 1).await;
    assert!(f
        .artifacts
        .read(&new.reference, new.owner)
        .await
        .unwrap()
        .is_some());
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_schema_two_is_not_upgraded_or_used_as_a_fallback() {
    let f = Fixture::ready().await;
    let legacy = f
        .auth
        .source
        .create_session_manual_task(
            &f.auth.learner_token,
            &f.base.task_workspace,
            id(),
            "Schema-two manual task",
        )
        .await
        .unwrap();
    let workspace = SessionTaskContentWorkspace::new(
        &f.base.task_schema,
        scope(),
        f.artifact_workspace.clone(),
    )
    .unwrap();
    let route = build_task_content_router(
        TaskContentHttpState::new(f.auth.source.clone(), workspace, ORIGIN).unwrap(),
    );
    for req in [
        request(
            "GET",
            &task_path(legacy.reference.task_id),
            &f.auth.learner_token,
            Body::empty(),
        ),
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(id(), &manifest("Not auto installed", vec![])),
        ),
    ] {
        error(
            route.clone(),
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
    assert_eq!(
        f.base
            .tasks
            .get(legacy.reference, legacy.owner)
            .await
            .unwrap(),
        Some(legacy)
    );
    assert_eq!(f.base.count_task("collaboration_tasks").await, 1);
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    let version: i32 = query("SELECT version FROM collaboration_task_schema")
        .fetch_one(&f.base.task_pool)
        .await
        .unwrap()
        .get("version");
    assert_eq!(version, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_competing_edits_have_one_success_and_one_stale_response() {
    let (f, task, old, new) = content_fixture::pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let path = format!("{}/content", task_path(task.task.reference.task_id));
    let route = app(&f);
    let first = request(
        "PUT",
        &path,
        &f.auth.learner_token,
        replace_body(&task, &manifest("Winner", vec![new.reference])),
    );
    let first = tokio::spawn(async move { response(route, first, StatusCode::OK).await });
    let first_pid = blocked_by(&f, blocker).await;
    let route = app(&f);
    let second = request(
        "PUT",
        &path,
        &f.auth.learner_token,
        replace_body(&task, &manifest("Loser", vec![old.reference])),
    );
    let second = tokio::spawn(async move {
        error(
            route,
            second,
            StatusCode::CONFLICT,
            "stale_revision",
            "unconfirmed",
        )
        .await
    });
    blocked_by(&f, first_pid).await;
    holder.rollback().await.unwrap();
    let saved = snapshot(&first.await.unwrap());
    second.await.unwrap();
    assert_eq!(saved.manifest.title(), "Winner");
    assert_eq!(u64::from(saved.task.revision), 2);
    f.unchanged(&saved, 2).await;
    f.assert_pool_restored().await;
    f.cleanup().await;
}
