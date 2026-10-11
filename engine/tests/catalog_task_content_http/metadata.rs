use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_full_metadata_binding_drift_and_unknown_exact_coordinates_never_write(
) {
    let (f, original, route, bound, _, _) = pair().await;
    let definitions = values();
    let unbound = create(
        &f,
        &route,
        &definitions[1],
        &manifest("Manual assessment catalogue task", vec![]),
    )
    .await;
    let unrelated = create(
        &f,
        &route,
        &definitions[2],
        &manifest("Another package still readable", vec![]),
    )
    .await;
    for field in 0..8 {
        let mut drifted = definitions.clone();
        let target = if field == 7 { &unbound } else { &bound };
        match field {
            0 => drifted[0].title = "Changed title".into(),
            1 => drifted[0].summary = "Changed summary".into(),
            2 => {
                drifted[0].evidence_schema.as_mut().unwrap().name =
                    "sample.document.other".parse().unwrap()
            }
            3 => drifted[0].evidence_schema.as_mut().unwrap().version = 2.try_into().unwrap(),
            4 => {
                drifted[0].assessment_policy.as_mut().unwrap().name =
                    "sample.document.other".parse().unwrap()
            }
            5 => drifted[0].assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap(),
            6 => {
                drifted[0].evidence_schema = None;
                drifted[0].assessment_policy = None;
            }
            7 => {
                drifted[1].evidence_schema = definitions[0].evidence_schema.clone();
                drifted[1].assessment_policy = definitions[0].assessment_policy.clone();
            }
            _ => unreachable!(),
        }
        assert_ne!(
            drifted, definitions,
            "effective full metadata / both binding direction injection"
        );
        assert_eq!(target.task.status, TaskStatus::Draft);
        let workspace = workspace(&f, &drifted);
        let changed = app(&f, &workspace);
        for req in saved_requests(target, &f.auth.learner_token) {
            denied(
                &f,
                &changed,
                req,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
        }
        read(&f, &changed, &f.auth.learner_token, &unrelated).await;
    }
    let missing_workspace = workspace(&f, &definitions[2..]);
    let missing = app(&f, &missing_workspace);
    for task in [&bound, &unbound] {
        for req in saved_requests(task, &f.auth.learner_token) {
            denied(
                &f,
                &missing,
                req,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
        }
    }
    read(&f, &missing, &f.auth.learner_token, &unrelated).await;
    for coordinate in 0..4 {
        let mut reference = definitions[0].reference.clone();
        match coordinate {
            0 => reference.package.name = "sample.unknown".parse().unwrap(),
            1 => reference.package.version = 9.try_into().unwrap(),
            2 => reference.name = "sample.document.missing".parse().unwrap(),
            3 => reference.version = 9.try_into().unwrap(),
            _ => unreachable!(),
        }
        assert_ne!(reference, definitions[0].reference);
        denied(
            &f,
            &route,
            request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(
                    id(),
                    &reference,
                    &manifest("No coordinate fallback", vec![]),
                ),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    denied(
        &f,
        &missing,
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(
                id(),
                &definitions[0].reference,
                &manifest("Missing allowlist", vec![]),
            ),
        ),
        StatusCode::BAD_REQUEST,
        "invalid_request",
    )
    .await;
    for task in [&bound, &unbound, &unrelated] {
        read(&f, &route, &f.auth.learner_token, task).await;
    }
    assert_eq!(f.count_task("collaboration_tasks").await, 3);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 3);
    f.assert_pool_restored().await;
    drop((missing, missing_workspace, route, original));
    f.cleanup().await;
}
