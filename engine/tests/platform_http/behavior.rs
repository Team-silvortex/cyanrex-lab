use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_create_read_preserves_exact_unicode_order_and_empty_payload() {
    let f = Fixture::ready().await;
    let first = f
        .create_artifact(&f.auth.learner_token, "\t中文😀\r\n".as_bytes())
        .await;
    let second = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            input_fixture::draft(b"Later revision"),
        )
        .await
        .unwrap();
    let value = manifest(
        " \u{202e}Unicode task😀 ",
        vec![second.reference.clone(), first.reference.clone()],
    );
    let task = create_http(&f, &value).await;
    assert_eq!(task.manifest, value);
    assert_eq!(task.task.owner, f.auth.learner.principal.reference);
    assert_eq!(task.task.reference.workspace, scope());
    assert_eq!(u64::from(task.task.revision), 1);
    assert_eq!(task.task.status, TaskStatus::Draft);
    assert!(task.task.definition.is_none());
    assert_eq!(
        task.task.input_refs,
        vec![second.reference.clone(), first.reference.clone()]
    );
    let read = response(
        app(&f),
        request(
            "GET",
            &task_path(task.task.reference.task_id),
            &f.auth.learner_token,
            Body::empty(),
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        read,
        json!({"schema_version":1,"task":task.task,"manifest":value,"contents":[{"artifact":second.reference,"text":"Later revision"},{"artifact":first.reference,"text":"\t中文😀\r\n"}]})
    );
    let empty = create_http(&f, &manifest("No code required", vec![])).await;
    let mut req = request(
        "GET",
        &task_path(empty.task.reference.task_id),
        &f.auth.learner_token,
        Body::empty(),
    );
    req.headers_mut().remove("origin");
    req.headers_mut().remove("content-type");
    let read = response(app(&f), req, StatusCode::OK).await;
    assert_eq!(read["contents"], json!([]));
    assert_eq!(read["manifest"], json!(empty.manifest));
    assert_eq!(f.count_task("collaboration_tasks").await, 2);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_replace_status_and_conflicts_return_exact_server_revisions() {
    let f = Fixture::ready().await;
    let old = f.create_artifact(&f.auth.learner_token, b"old").await;
    let new = f.create_artifact(&f.auth.learner_token, b"new").await;
    let original = create_http(&f, &manifest("Original", vec![old.reference.clone()])).await;
    let path = task_path(original.task.reference.task_id);
    let metadata = TaskContentManifest::new(
        "Renamed",
        vec![TextPayloadBinding::new(old.reference.clone(), "CON.rs", "future.language").unwrap()],
    )
    .unwrap();
    let changed = snapshot(
        &response(
            app(&f),
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&original, &metadata),
            ),
            StatusCode::OK,
        )
        .await,
    );
    assert_eq!(changed.manifest, metadata);
    assert_eq!(u64::from(changed.task.revision), 2);
    assert_eq!(changed.task.created_at, original.task.created_at);
    error(
        app(&f),
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(original.task.reference.task_id, &metadata),
        ),
        StatusCode::CONFLICT,
        "conflict",
        "unconfirmed",
    )
    .await;
    error(
        app(&f),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&original, &manifest("Stale", vec![])),
        ),
        StatusCode::CONFLICT,
        "stale_revision",
        "unconfirmed",
    )
    .await;
    error(
        app(&f),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&changed, &changed.manifest),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "unconfirmed",
    )
    .await;
    let reordered = manifest(
        "Reordered",
        vec![new.reference.clone(), old.reference.clone()],
    );
    let updated = snapshot(
        &response(
            app(&f),
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&changed, &reordered),
            ),
            StatusCode::OK,
        )
        .await,
    );
    assert_eq!(updated.task.input_refs, vec![new.reference, old.reference]);
    assert_eq!(u64::from(updated.task.revision), 3);
    let ready = snapshot(
        &response(
            app(&f),
            request(
                "POST",
                &format!("{path}/status"),
                &f.auth.learner_token,
                json!({"schema_version":1,"expected_revision":3,"status":"ready"}).to_string(),
            ),
            StatusCode::OK,
        )
        .await,
    );
    assert_eq!(ready.manifest, reordered);
    assert_eq!(ready.task.status, TaskStatus::Ready);
    assert_eq!(u64::from(ready.task.revision), 4);
    error(
        app(&f),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&ready, &manifest("Not draft", vec![])),
        ),
        StatusCode::CONFLICT,
        "invalid_transition",
        "unconfirmed",
    )
    .await;
    error(
        app(&f),
        request(
            "POST",
            &format!("{path}/status"),
            &f.auth.learner_token,
            json!({"schema_version":1,"expected_revision":3,"status":"cancelled"}).to_string(),
        ),
        StatusCode::CONFLICT,
        "stale_revision",
        "unconfirmed",
    )
    .await;
    f.unchanged(&ready, 4).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_private_ownership_and_forged_pins_never_expose_content() {
    let (f, task, old, _) = content_fixture::pair().await;
    let path = task_path(task.task.reference.task_id);
    for task_id in [task.task.reference.task_id, id()] {
        error(
            app(&f),
            request(
                "GET",
                &task_path(task_id),
                &f.auth.manager_token,
                Body::empty(),
            ),
            StatusCode::NOT_FOUND,
            "not_found",
            "unconfirmed",
        )
        .await;
    }
    error(
        app(&f),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.manager_token,
            replace_body(&task, &manifest("Not shared", vec![])),
        ),
        StatusCode::NOT_FOUND,
        "not_found",
        "unconfirmed",
    )
    .await;
    error(
        app(&f),
        request(
            "POST",
            &format!("{path}/status"),
            &f.auth.manager_token,
            json!({"schema_version":1,"expected_revision":1,"status":"ready"}).to_string(),
        ),
        StatusCode::NOT_FOUND,
        "not_found",
        "unconfirmed",
    )
    .await;
    let other = f
        .create_artifact(&f.auth.manager_token, b"Private manager bytes")
        .await;
    error(
        app(&f),
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(id(), &manifest("Foreign owner", vec![other.reference])),
        ),
        StatusCode::NOT_FOUND,
        "not_found",
        "unconfirmed",
    )
    .await;
    let mut forged = old.reference.clone();
    forged.sha256 = "b".repeat(64).parse().unwrap();
    error(
        app(&f),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&task, &manifest("Forged digest", vec![forged])),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    let mut foreign = old.reference;
    foreign.workspace.workspace_id = id();
    error(
        app(&f),
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(id(), &manifest("Foreign scope", vec![foreign])),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "unconfirmed",
    )
    .await;
    f.unchanged(&task, 1).await;
    assert_eq!(f.count_task("collaboration_tasks").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_preflight_rejections_cannot_change_a_valid_session_task() {
    let f = Fixture::ready().await;
    let task = create_http(&f, &manifest("Original", vec![])).await;
    let path = format!("{}/content", task_path(task.task.reference.task_id));
    let body = replace_body(&task, &manifest("Would edit", vec![]));
    // A premature authorization lookup would block here, even if its result were discarded.
    let mut blocker = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let admission = tokio::time::timeout(Duration::from_secs(1), async {
        for missing in [MARKER, "origin"] {
            let mut req = request("PUT", &path, &f.auth.learner_token, body.clone());
            req.headers_mut().remove(missing);
            error(
                app(&f),
                req,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            )
            .await;
        }
        let mut legacy = request("PUT", &path, &f.auth.learner_token, body.clone());
        legacy.headers_mut().insert(
            "cookie",
            format!("cyanrex_session={}", f.auth.learner_token)
                .parse()
                .unwrap(),
        );
        error(
            app(&f),
            legacy,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        )
        .await;
        let mut duplicate = request("PUT", &path, &f.auth.learner_token, body);
        duplicate.headers_mut().append(
            "cookie",
            format!("{COOKIE}={}", f.auth.learner_token)
                .parse()
                .unwrap(),
        );
        error(
            app(&f),
            duplicate,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        )
        .await;
        error(
            app(&f),
            request("PUT", &path, &f.auth.learner_token, "[1,1,{}]"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
    })
    .await;
    blocker.rollback().await.unwrap();
    admission.expect("a rejected HTTP request entered the blocked durable Session source");
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    f.unchanged(&task, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_new_invalid_text_and_corrupt_old_bytes_cannot_be_saved_or_erased() {
    let f = Fixture::ready().await;
    let task = create_http(&f, &manifest("Empty original", vec![])).await;
    let path = format!("{}/content", task_path(task.task.reference.task_id));
    for bytes in [vec![0xff], b"bad\0text".to_vec(), vec![b'x'; 262145]] {
        let artifact = f.create_artifact(&f.auth.learner_token, &bytes).await;
        let bad = manifest("Invalid text", vec![artifact.reference]);
        error(
            app(&f),
            request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(id(), &bad),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "unconfirmed",
        )
        .await;
        error(
            app(&f),
            request(
                "PUT",
                &path,
                &f.auth.learner_token,
                replace_body(&task, &bad),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "unconfirmed",
        )
        .await;
    }
    let old = f
        .create_artifact(&f.auth.learner_token, b"Stored content")
        .await;
    let populated = create_http(
        &f,
        &manifest("Keep validation", vec![old.reference.clone()]),
    )
    .await;
    fs::remove_file(f.blob(&old.reference)).unwrap();
    let path = task_path(populated.task.reference.task_id);
    for req in [
        request("GET", &path, &f.auth.learner_token, Body::empty()),
        request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&populated, &manifest("Remove broken old content", vec![])),
        ),
        request(
            "POST",
            &format!("{path}/status"),
            &f.auth.learner_token,
            json!({"schema_version":1,"expected_revision":1,"status":"cancelled"}).to_string(),
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
    f.unchanged(&task, 2).await;
    f.unchanged(&populated, 2).await;
    assert_eq!(f.count_task("collaboration_tasks").await, 2);
    f.cleanup().await;
}
