use super::*;
use axum::body::HttpBody;
use index_sql_fixture::{backend, blocked_by, create_at, lock_task, snapshot};

async fn saturated(f: &Fixture, route: &Router) {
    tokio::time::timeout(
        Duration::from_secs(1),
        denied(
            f,
            route,
            request_with_token("GET", COLLECTION, &f.auth.learner_token, unread_body()),
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("full HTTP budget must not queue, poll the body or dispatch SQL");
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_real_wait_cancellation_and_response_lifetime_release_capacity() {
    for catalogue in [false, true] {
        let f = Fixture::ready().await;
        let definitions = values();
        let task = create_at(
            &f,
            10,
            catalogue.then_some(&definitions[1]),
            &f.auth.learner_token,
            manifest("Cancellation and retained metadata", vec![]),
        )
        .await;
        let workspace = if catalogue {
            configured(&f, &definitions)
        } else {
            index(&f)
        };
        let route = bounded_app(&f, &workspace);
        assert_page(
            &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
            &[task.clone()],
            None,
        );
        let before = snapshot(&f).await;
        let mut holder = lock_task(&f, task.task.reference).await;
        let blocker = backend(&mut holder).await;
        let req = request_with_token("GET", COLLECTION, &f.auth.learner_token, Body::empty());
        let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
        let waited = blocked_by(&f, blocker).await.is_some();
        assert!(!pending.0.is_finished());
        saturated(&f, &route).await;
        pending.0.abort();
        assert!((&mut pending.0).await.unwrap_err().is_cancelled());
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        assert!(
            waited,
            "cancel only after a demonstrated real owned-row wait"
        );
        assert_eq!(
            snapshot(&f).await,
            before,
            "cancellation fixture proves read-only rows and bytes, not a write receipt"
        );
        let response = route
            .clone()
            .oneshot(request_with_token(
                "GET",
                COLLECTION,
                &f.auth.learner_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        headers(&response);
        assert_eq!(snapshot(&f).await, before);
        saturated(&f, &route).await;
        let mut body = response.into_body();
        let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_page(
            &serde_json::from_slice::<Value>(&frame).unwrap(),
            &[task.clone()],
            None,
        );
        assert!(
            body.is_end_stream(),
            "last JSON frame retires the slot even while Body survives"
        );
        let response = route
            .clone()
            .oneshot(request_with_token(
                "GET",
                COLLECTION,
                &f.auth.learner_token,
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        headers(&response);
        assert_eq!(snapshot(&f).await, before);
        saturated(&f, &route).await;
        drop(response);
        assert_page(
            &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
            &[task],
            None,
        );
        assert_eq!(snapshot(&f).await, before);
        f.assert_pool_restored().await;
        drop((body, pending, route));
        f.cleanup().await;
    }
}
