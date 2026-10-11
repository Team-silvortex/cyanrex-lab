use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_metadata_does_not_read_artifact_namespace_or_blobs() {
    for missing in [false, true] {
        let f = Fixture::ready().await;
        let token = &f.auth.learner_token;
        let artifact = f
            .create_artifact(token, b"Private text must not appear in metadata")
            .await;
        let task = f.create(vec![artifact.reference.clone()]).await;
        let route = app(&f, &index(&f));
        let expected = read(&f, &route, token, COLLECTION).await;
        assert_page(&expected, &[task.clone()], None);
        let old = build_task_content_router(
            TaskContentHttpState::new(f.auth.source.clone(), f.workspace.clone(), ORIGIN).unwrap(),
        );
        let path = format!("/platform/v1/tasks/{}", task.task.reference.task_id);
        let before = index_sql_fixture::snapshot(&f).await;
        let original = response_value(
            old.clone()
                .oneshot(request_with_token("GET", &path, token, Body::empty()))
                .await
                .unwrap(),
            StatusCode::OK,
        )
        .await;
        assert_eq!(
            original["contents"][0]["text"],
            "Private text must not appear in metadata"
        );
        assert_eq!(index_sql_fixture::snapshot(&f).await, before);
        if missing {
            fs::remove_file(f.blob(&artifact.reference)).unwrap();
        } else {
            assert_eq!(
                query("UPDATE collaboration_artifact_schema SET version = 999")
                    .execute(&f.artifact_pool)
                    .await
                    .unwrap()
                    .rows_affected(),
                1
            );
        }
        assert_eq!(read(&f, &route, token, COLLECTION).await, expected);
        denied(
            &f,
            &old,
            request_with_token("GET", &path, token, Body::empty()),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
        f.assert_pool_restored().await;
        drop((route, old));
        f.cleanup().await;
    }
}
