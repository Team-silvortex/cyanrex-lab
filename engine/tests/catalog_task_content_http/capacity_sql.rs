use super::*;
use axum::body::HttpBody;

fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("capacity rejection must not poll body or attempt SQL")
        },
    ))
}
async fn saturated(f: &Fixture, route: &Router, req: Request<Body>) {
    let before = observed(f).await;
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            route.clone(),
            req,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("capacity must never queue");
    assert_eq!(observed(f).await, before);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_shared_budget_covers_committed_body_sql_wait_cancel_and_eof() {
    for cancel in [false, true] {
        let (f, workspace, _, task, _, new) = pair().await;
        let route = bounded_app(&f, &workspace);
        let path = task_path(task.task.reference.task_id);
        let created = route
            .clone()
            .oneshot(request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(
                    id(),
                    &values()[1].reference,
                    &manifest("Held committed body", vec![]),
                ),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        headers(&created);
        for (method, target) in [
            ("GET", path.clone()),
            ("POST", COLLECTION.into()),
            ("PUT", format!("{path}/content")),
            ("POST", format!("{path}/status")),
        ] {
            saturated(
                &f,
                &route,
                request(method, &target, &f.auth.learner_token, unread_body()),
            )
            .await;
        }
        let mut body = created.into_body();
        let bytes = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert!(body.is_end_stream());
        let held_task = snapshot(&serde_json::from_slice(&bytes).unwrap());
        read(&f, &route, &f.auth.learner_token, &held_task).await;
        assert!(body.frame().await.is_none());
        // The completed Body remains alive. Its last frame / EOF, not just Drop, frees admission.
        let before = observed(&f).await;
        let mut holder = f.pause_task_event().await;
        let blocker = backend(&mut holder).await;
        let req = request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(
                &task,
                &manifest("Committed replacement", vec![new.reference.clone()]),
            ),
        );
        let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
        blocked_by(&f, blocker).await;
        saturated(
            &f,
            &route,
            request("GET", &path, &f.auth.learner_token, unread_body()),
        )
        .await;
        let expected = if cancel {
            pending.0.abort();
            assert!((&mut pending.0).await.unwrap_err().is_cancelled());
            f.assert_pool_restored().await;
            holder.rollback().await.unwrap();
            // Rollback is established by the fixture's observed state, never inferred from HTTP.
            assert_eq!(observed(&f).await, before);
            task.clone()
        } else {
            holder.rollback().await.unwrap();
            let response = tokio::time::timeout(Duration::from_secs(3), &mut pending.0)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            headers(&response);
            saturated(
                &f,
                &route,
                request("GET", &path, &f.auth.learner_token, unread_body()),
            )
            .await;
            snapshot(
                &serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap(),
            )
        };
        assert_eq!(expected.task.definition, task.task.definition);
        read(&f, &route, &f.auth.learner_token, &expected).await;
        let held = route
            .clone()
            .oneshot(request("GET", &path, &f.auth.learner_token, Body::empty()))
            .await
            .unwrap();
        assert_eq!(held.status(), StatusCode::OK);
        saturated(
            &f,
            &route,
            request("GET", &path, &f.auth.learner_token, unread_body()),
        )
        .await;
        drop(held);
        read(&f, &route, &f.auth.learner_token, &expected).await;
        f.assert_pool_restored().await;
        drop((body, pending, route, workspace));
        f.cleanup().await;
    }
}
