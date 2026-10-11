use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_mixed_frozen_packages_versions_and_bindings_roundtrip_all_four_commands(
) {
    let f = Fixture::ready().await;
    let definitions = values();
    let workspace = workspace(&f, &definitions);
    let route = app(&f, &workspace);
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
    let empty_text = f.create_artifact(&f.auth.learner_token, b"").await;
    for (index, definition) in definitions.iter().enumerate() {
        let pins = match index {
            0 => vec![second.reference.clone(), first.reference.clone()],
            1 => vec![],
            _ => vec![empty_text.reference.clone()],
        };
        let initial = manifest(" \u{202e}Private task😀 ", pins);
        let original = create(&f, &route, definition, &initial).await;
        assert_eq!(original.task.definition, Some(definition.clone()));
        assert_eq!(original.task.owner, f.auth.learner.principal.reference);
        assert_eq!(original.task.status, TaskStatus::Draft);
        assert_eq!(u64::from(original.task.revision), 1);
        let readback = read(&f, &route, &f.auth.learner_token, &original).await;
        let expected = match index {
            0 => {
                json!([{"artifact":second.reference,"text":"Later revision"},{"artifact":first.reference,"text":"\t中文😀\r\n"}])
            }
            1 => json!([]),
            _ => json!([{"artifact":empty_text.reference,"text":""}]),
        };
        assert_eq!(readback["contents"], expected);
        let path = task_path(original.task.reference.task_id);
        let replacement = manifest(
            "Retained frozen metadata",
            vec![empty_text.reference.clone(), first.reference.clone()],
        );
        let changed = snapshot(
            &response(
                route.clone(),
                request(
                    "PUT",
                    &format!("{path}/content"),
                    &f.auth.learner_token,
                    replace_body(&original, &replacement),
                ),
                StatusCode::OK,
            )
            .await,
        );
        assert_eq!(changed.task.definition, Some(definition.clone()));
        assert_eq!(changed.task.created_at, original.task.created_at);
        assert_eq!(changed.manifest, replacement);
        assert_eq!(u64::from(changed.task.revision), 2);
        for (req, code, status) in [
            (
                request(
                    "POST",
                    COLLECTION,
                    &f.auth.learner_token,
                    create_body(
                        original.task.reference.task_id,
                        &definition.reference,
                        &initial,
                    ),
                ),
                "conflict",
                StatusCode::CONFLICT,
            ),
            (
                request(
                    "PUT",
                    &format!("{path}/content"),
                    &f.auth.learner_token,
                    replace_body(&original, &manifest("Stale", vec![])),
                ),
                "stale_revision",
                StatusCode::CONFLICT,
            ),
            (
                request(
                    "PUT",
                    &format!("{path}/content"),
                    &f.auth.learner_token,
                    replace_body(&changed, &changed.manifest),
                ),
                "invalid_request",
                StatusCode::BAD_REQUEST,
            ),
        ] {
            denied(&f, &route, req, status, code).await;
        }
        let ready = snapshot(
            &response(
                route.clone(),
                request(
                    "POST",
                    &format!("{path}/status"),
                    &f.auth.learner_token,
                    status_body(&changed, "ready"),
                ),
                StatusCode::OK,
            )
            .await,
        );
        assert_eq!(ready.task.definition, Some(definition.clone()));
        assert_eq!(ready.task.status, TaskStatus::Ready);
        assert_eq!(u64::from(ready.task.revision), 3);
        denied(
            &f,
            &route,
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&ready, &manifest("No overwrite", vec![])),
            ),
            StatusCode::CONFLICT,
            "invalid_transition",
        )
        .await;
        denied(
            &f,
            &route,
            request(
                "POST",
                &format!("{path}/status"),
                &f.auth.learner_token,
                status_body(&changed, "cancelled"),
            ),
            StatusCode::CONFLICT,
            "stale_revision",
        )
        .await;
        read(&f, &route, &f.auth.learner_token, &ready).await;
    }
    assert_eq!(f.count_task("collaboration_tasks").await, 3);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 9);
    assert_eq!(f.files(), 3);
    f.assert_pool_restored().await;
    drop((route, workspace));
    f.cleanup().await;
}
