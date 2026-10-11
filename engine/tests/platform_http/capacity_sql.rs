use super::*;

fn bounded_app(f: &Fixture) -> Router {
    build_task_content_router(
        TaskContentHttpState::with_capacity(f.auth.source.clone(), f.workspace.clone(), ORIGIN, 1)
            .unwrap(),
    )
}

fn unread_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("capacity rejection must not poll input or reach the SQL command")
        },
    ))
}

async fn saturated(route: &Router, req: Request<Body>) {
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
    .expect("full admission queued instead of rejecting");
}

type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
struct Pending(tokio::task::JoinHandle<Result<axum::response::Response, std::convert::Infallible>>);
impl Drop for Pending {
    fn drop(&mut self) {
        // An early assertion failure must not detach an admitted SQL request from its fixture.
        self.0.abort();
    }
}
async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
async fn blocked_by(f: &Fixture, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let count: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid))")
                .bind(blocker).fetch_one(&f.auth.base.admin).await.unwrap().get("n");
            if count == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("mutation did not reach the exact event-write barrier");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_capacity_retains_committed_content_response_until_body_end_or_drop()
{
    use axum::body::HttpBody;
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, "\t中文😀\r\n".as_bytes())
        .await;
    let value = manifest("Private bounded response", vec![artifact.reference.clone()]);
    let task_id = id();
    let path = task_path(task_id);
    let route = bounded_app(&f);
    let created = route
        .clone()
        .oneshot(request(
            "POST",
            COLLECTION,
            &f.auth.learner_token,
            create_body(task_id, &value),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    headers(&created);
    // The transaction has committed, but even a small unconsumed response keeps the shared slot.
    assert_eq!(f.count_task("collaboration_tasks").await, 1);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    for (method, target) in [
        ("GET", path.clone()),
        ("POST", COLLECTION.to_string()),
        ("PUT", format!("{path}/content")),
        ("POST", format!("{path}/status")),
    ] {
        saturated(
            &route,
            request(method, &target, &f.auth.learner_token, unread_body()),
        )
        .await;
    }
    let mut body = created.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(
        body.is_end_stream(),
        "last JSON frame must retire the response slot"
    );
    let committed = snapshot(&serde_json::from_slice(&frame).unwrap());
    assert_eq!(committed.task.reference.task_id, task_id);
    assert_eq!(committed.manifest, value);
    // Retain the completed Body object: release must not depend solely on its destructor.
    let read = route
        .clone()
        .oneshot(request("GET", &path, &f.auth.learner_token, Body::empty()))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    saturated(
        &route,
        request("GET", &path, &f.auth.learner_token, unread_body()),
    )
    .await;
    drop(read);
    let read = response(
        route.clone(),
        request("GET", &path, &f.auth.learner_token, Body::empty()),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        read["contents"],
        json!([{"artifact":artifact.reference,"text":"\t中文😀\r\n"}])
    );
    f.unchanged(&committed, 1).await;
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    drop(body);
    drop(route);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_platform_http_capacity_spans_mutation_wait_and_releases_after_cancel_or_commit() {
    for cancel in [false, true] {
        let f = Fixture::ready().await;
        let task = create_http(&f, &manifest("Original", vec![])).await;
        let path = task_path(task.task.reference.task_id);
        let mut holder = f.pause_task_event().await;
        let blocker = backend(&mut holder).await;
        let route = bounded_app(&f);
        let req = request(
            "PUT",
            &format!("{path}/content"),
            &f.auth.learner_token,
            replace_body(&task, &manifest("Confirmed new content", vec![])),
        );
        let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
        blocked_by(&f, blocker).await;
        assert!(!pending.0.is_finished());
        saturated(
            &route,
            request("POST", COLLECTION, &f.auth.learner_token, unread_body()),
        )
        .await;
        saturated(
            &route,
            request("GET", &path, &f.auth.learner_token, unread_body()),
        )
        .await;
        let expected = if cancel {
            pending.0.abort();
            assert!((&mut pending.0).await.unwrap_err().is_cancelled());
            f.assert_pool_restored().await;
            holder.rollback().await.unwrap();
            // Only this observed rollback fixture establishes unchanged state, not cancellation alone.
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
                &route,
                request("GET", &path, &f.auth.learner_token, unread_body()),
            )
            .await;
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let result = snapshot(&serde_json::from_slice(&bytes).unwrap());
            assert_eq!(u64::from(result.task.revision), 2);
            assert_eq!(result.manifest.title(), "Confirmed new content");
            result
        };
        let read = response(
            route.clone(),
            request("GET", &path, &f.auth.learner_token, Body::empty()),
            StatusCode::OK,
        )
        .await;
        assert_eq!(read["task"], json!(expected.task));
        assert_eq!(read["manifest"], json!(expected.manifest));
        assert_eq!(read["contents"], json!([]));
        f.unchanged(&expected, if cancel { 1 } else { 2 }).await;
        assert_eq!(f.count_task("collaboration_tasks").await, 1);
        assert_eq!(f.files(), 0);
        f.assert_pool_restored().await;
        drop(route);
        f.cleanup().await;
    }
}
