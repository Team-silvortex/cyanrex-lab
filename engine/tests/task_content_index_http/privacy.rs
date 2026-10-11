use super::*;
use index_sql_fixture::create_at;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_teacher_isolation_and_untrusted_query_controls_are_read_only() {
    let f = Fixture::ready().await;
    let route = app(&f, &index(&f));
    let learner = create_at(
        &f,
        10,
        None,
        &f.auth.learner_token,
        manifest("Learner private", vec![]),
    )
    .await;
    let teacher = create_at(
        &f,
        20,
        None,
        &f.auth.manager_token,
        manifest("Teacher private", vec![]),
    )
    .await;
    assert_page(
        &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
        &[learner.clone()],
        None,
    );
    assert_page(
        &read(&f, &route, &f.auth.manager_token, COLLECTION).await,
        &[teacher.clone()],
        None,
    );
    for field in [
        "owner",
        "principal_id",
        "namespace",
        "workspace",
        "definition_ref",
        "mode",
        "schema_version",
        "token",
    ] {
        denied(
            &f,
            &route,
            request_with_token(
                "GET",
                &format!("{COLLECTION}?{field}=teacher"),
                &f.auth.manager_token,
                unread_body(),
            ),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        )
        .await;
    }
    // A teacher credential does not broaden private ownership, including a same-workspace cursor.
    assert_page(
        &read(
            &f,
            &route,
            &f.auth.manager_token,
            &format!("{COLLECTION}?after={}", learner.task.reference.task_id),
        )
        .await,
        &[teacher.clone()],
        None,
    );
    assert_eq!(
        query("UPDATE collaboration_tasks SET snapshot = '{}' WHERE task_id = $1")
            .bind(learner.task.reference.task_id.as_uuid())
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_page(
        &read(&f, &route, &f.auth.manager_token, COLLECTION).await,
        &[teacher],
        None,
    );
    denied(
        &f,
        &route,
        request_with_token("GET", COLLECTION, &f.auth.learner_token, Body::empty()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    denied(
        &f,
        &route,
        request_with_token("GET", COLLECTION, &f.auth.target_token, Body::empty()),
        StatusCode::FORBIDDEN,
        "access_denied",
        "unconfirmed",
    )
    .await;
    f.auth.bind_target().await;
    denied(
        &f,
        &route,
        request_with_token("GET", COLLECTION, &f.auth.target_token, Body::empty()),
        StatusCode::FORBIDDEN,
        "access_denied",
        "unconfirmed",
    )
    .await;
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}
