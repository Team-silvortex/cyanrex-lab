use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_manual_catalog_and_owner_privacy_are_bidirectionally_disjoint() {
    let (f, workspace, route, task, old, _) = pair().await;
    let unbound = create(
        &f,
        &route,
        &values()[1],
        &manifest("Unbound is not manual", vec![]),
    )
    .await;
    let manual = manual_http_fixture::create_http(
        &f,
        &manifest("Manual private text", vec![old.reference.clone()]),
    )
    .await;
    let manual_route = manual_http_fixture::app(&f);
    for hidden in [&task, &unbound] {
        let path = manual_http_fixture::task_path(hidden.task.reference.task_id);
        for req in [
            request("GET", &path, &f.auth.learner_token, Body::empty()),
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(hidden, &manifest("No mode rewrite", vec![])),
            ),
            request(
                "POST",
                &format!("{path}/status"),
                &f.auth.learner_token,
                status_body(hidden, "ready"),
            ),
        ] {
            denied(
                &f,
                &manual_route,
                req,
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
            )
            .await;
        }
    }
    for req in saved_requests(&manual, &f.auth.learner_token) {
        denied(
            &f,
            &route,
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    let manager_task = snapshot(
        &response(
            route.clone(),
            request(
                "POST",
                COLLECTION,
                &f.auth.manager_token,
                create_body(
                    id(),
                    &values()[2].reference,
                    &manifest("Manager own task", vec![]),
                ),
            ),
            StatusCode::CREATED,
        )
        .await,
    );
    for (task, token) in [
        (&task, &f.auth.manager_token),
        (&unbound, &f.auth.manager_token),
        (&manager_task, &f.auth.learner_token),
    ] {
        for req in saved_requests(task, token) {
            denied(&f, &route, req, StatusCode::NOT_FOUND, "not_found").await;
        }
        denied(
            &f,
            &route,
            request("GET", &task_path(id()), token, Body::empty()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    let manager_private = f
        .create_artifact(&f.auth.manager_token, b"Private manager bytes")
        .await;
    for (token, reference) in [
        (&f.auth.manager_token, old.reference.clone()),
        (&f.auth.learner_token, manager_private.reference.clone()),
    ] {
        denied(
            &f,
            &route,
            request(
                "POST",
                COLLECTION,
                token,
                create_body(
                    id(),
                    &values()[0].reference,
                    &manifest("Foreign text", vec![reference]),
                ),
            ),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    let mut forged = old.reference.clone();
    forged.sha256 = "b".repeat(64).parse().unwrap();
    denied(
        &f,
        &route,
        request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(
                id(),
                &values()[0].reference,
                &manifest("Forged pin", vec![forged]),
            ),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
    )
    .await;
    read(&f, &route, &f.auth.learner_token, &task).await;
    read(&f, &route, &f.auth.learner_token, &unbound).await;
    read(&f, &route, &f.auth.manager_token, &manager_task).await;
    f.unchanged(&manual, 4).await;
    f.assert_pool_restored().await;
    drop((manual_route, route, workspace));
    f.cleanup().await;
}
