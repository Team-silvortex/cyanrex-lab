use super::*;
use sha2::{Digest, Sha256};

pub(super) fn reference(
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
pub(super) fn revision(value: &Value, reading: bool) -> ArtifactRevision {
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value.as_object().unwrap().len(),
        if reading { 3 } else { 2 }
    );
    serde_json::from_value(value["artifact"].clone()).unwrap()
}
pub(super) async fn publish(f: &ContentFixture, text: &str) -> ArtifactRevision {
    revision(
        &response(
            app(f),
            request(
                "POST",
                TEXT_COLLECTION,
                &f.auth.learner_token,
                create_body(id(), id(), "Private named content", text),
            ),
            StatusCode::CREATED,
        )
        .await,
        false,
    )
}
pub(super) async fn counts(f: &ContentFixture, expected: i64) {
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        assert_eq!(f.count_artifact(table).await, expected, "{table}");
    }
}
pub(super) async fn decode(response: axum::response::Response, status: StatusCode) -> Value {
    headers(&response);
    assert_eq!(response.status(), status);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
pub(super) async fn failure(response: axum::response::Response, status: StatusCode, code: &str) {
    assert_eq!(
        decode(response, status).await,
        json!({"schema_version":1,"error":{"code":code,"outcome":"unconfirmed"}})
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_roundtrip_and_task_manifest_bridge_preserve_exact_text() {
    let f = ContentFixture::ready().await;
    let mut pins = Vec::new();
    let texts = [
        String::new(),
        "\t中文😀e\u{301}\r\n<script>inert</script>".into(),
        "x".repeat(256 * 1024),
    ];
    for text in &texts {
        let artifact_id = id();
        let revision_id = id();
        let title = "  Explicit Task title😀  ";
        let created = revision(
            &response(
                app(&f),
                request(
                    "POST",
                    TEXT_COLLECTION,
                    &f.auth.learner_token,
                    create_body(artifact_id, revision_id, title, text),
                ),
                StatusCode::CREATED,
            )
            .await,
            false,
        );
        assert_eq!(
            created.reference,
            reference(artifact_id, revision_id, text.as_bytes())
        );
        assert_eq!(created.owner, f.auth.learner.principal.reference);
        assert_eq!(created.title, title);
        assert_eq!(created.kind.as_str(), "cyanrex.task-text");
        assert_eq!(created.media_type, "text/plain");
        assert_eq!(created.byte_length, text.len() as u64);
        assert_eq!(u64::from(created.sequence), 1);
        assert_eq!(created.parent, None);
        let read = response(
            app(&f),
            request(
                "GET",
                &text_path(&created.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            StatusCode::OK,
        )
        .await;
        assert_eq!(revision(&read, true), created);
        assert_eq!(read["text"], *text);
        pins.push(created.reference);
    }
    let value = manifest("Separate Task command", pins.clone());
    let task = http_fixture::create_http(&f, &value).await;
    let read = http_fixture::response(
        http_fixture::app(&f),
        http_fixture::request(
            "GET",
            &http_fixture::task_path(task.task.reference.task_id),
            &f.auth.learner_token,
            Body::empty(),
        ),
        StatusCode::OK,
    )
    .await;
    assert_eq!(read["manifest"], json!(value));
    assert_eq!(
        read["contents"],
        json!(pins
            .iter()
            .zip(&texts)
            .map(|(artifact, text)| json!({"artifact":artifact,"text":text}))
            .collect::<Vec<_>>())
    );
    counts(&f, 3).await;
    f.unchanged(&task, 1).await;
    assert_eq!(f.files(), 3);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_reads_exact_history_and_rejects_wrong_digest() {
    let f = ContentFixture::ready().await;
    let first = publish(&f, "original").await;
    let next = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            ArtifactDraft::new(
                "cyanrex.task-text".parse().unwrap(),
                "A later title",
                "text/plain",
                b"later".to_vec(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    for (saved, text) in [(&first, "original"), (&next, "later")] {
        let read = response(
            app(&f),
            request(
                "GET",
                &text_path(&saved.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            StatusCode::OK,
        )
        .await;
        assert_eq!(revision(&read, true), *saved);
        assert_eq!(read["text"], text);
    }
    assert_eq!(next.parent, Some(first.reference.clone()));
    let mut wrong = first.reference.clone();
    wrong.sha256 = "0".repeat(64).parse().unwrap();
    error(
        app(&f),
        request(
            "GET",
            &text_path(&wrong),
            &f.auth.learner_token,
            Body::empty(),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    assert_eq!(f.count_artifact("collaboration_artifacts").await, 1);
    assert_eq!(
        f.count_artifact("collaboration_artifact_revisions").await,
        2
    );
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_preflight_and_private_owner_do_not_probe_or_write() {
    let f = ContentFixture::ready().await;
    // A premature Session lookup would wait here even if the handler discarded its result.
    let mut blocker = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let admission = tokio::time::timeout(Duration::from_secs(1), async {
        for field in [
            "owner",
            "workspace",
            "sha256",
            "kind",
            "media_type",
            "parent",
            "path",
        ] {
            let mut forged: Value =
                serde_json::from_str(&create_body(id(), id(), "Named", "private")).unwrap();
            forged[field] = json!("untrusted");
            error(
                app(&f),
                request(
                    "POST",
                    TEXT_COLLECTION,
                    &f.auth.learner_token,
                    forged.to_string(),
                ),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
        }
        for (title, text) in [(" ", "private"), ("Named", "\0")] {
            error(
                app(&f),
                request(
                    "POST",
                    TEXT_COLLECTION,
                    &f.auth.learner_token,
                    create_body(id(), id(), title, text),
                ),
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            )
            .await;
        }
    })
    .await;
    blocker.rollback().await.unwrap();
    admission.expect("a rejected text request entered the blocked durable Session source");
    counts(&f, 0).await;
    assert_eq!(f.files(), 0);
    let first = publish(&f, "private bytes").await;
    // Even a manager who knows the exact pin cannot distinguish this missing blob from absence.
    fs::remove_file(f.blob(&first.reference)).unwrap();
    for pin in [first.reference.clone(), reference(id(), id(), b"absent")] {
        error(
            app(&f),
            request(
                "GET",
                &text_path(&pin),
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
            "GET",
            &text_path(&first.reference),
            &f.auth.learner_token,
            Body::empty(),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    counts(&f, 1).await;
    assert_eq!(f.files(), 0);
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_text_artifact_http_valid_generic_artifacts_outside_text_profile_remain_private() {
    let f = ContentFixture::ready().await;
    let cases = [
        ("sample.document", "text/plain", b"valid text".to_vec()),
        ("cyanrex.task-text", "application/json", b"{}".to_vec()),
        ("cyanrex.task-text", "text/plain", vec![255]),
        ("cyanrex.task-text", "text/plain", b"bad\0text".to_vec()),
        (
            "cyanrex.task-text",
            "text/plain",
            "bad\u{85}text".as_bytes().to_vec(),
        ),
        (
            "cyanrex.task-text",
            "text/plain",
            vec![b'x'; 256 * 1024 + 1],
        ),
    ];
    for (kind, media, bytes) in cases {
        let saved = f
            .auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                id(),
                id(),
                ArtifactDraft::new(
                    kind.parse().unwrap(),
                    "Generic private content",
                    media,
                    bytes.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        error(
            app(&f),
            request(
                "GET",
                &text_path(&saved.reference),
                &f.auth.learner_token,
                Body::empty(),
            ),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
        let actual = f
            .auth
            .source
            .read_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                &saved.reference,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.revision, saved);
        assert_eq!(actual.bytes, bytes);
    }
    counts(&f, 6).await;
    assert_eq!(f.files(), 6);
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    f.assert_pool_restored().await;
    f.cleanup().await;
}
