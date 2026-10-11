use super::*;
use index_sql_fixture::{
    backend, blocked_by, create_at, expire_soon, lock_task, snapshot, wait_expired,
};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_real_row_wait_expiry_discards_metadata() {
    for catalogue in [false, true] {
        let f = Fixture::ready().await;
        let definitions = values();
        let task = create_at(
            &f,
            10,
            catalogue.then_some(&definitions[0]),
            &f.auth.learner_token,
            manifest("Expiry target", vec![]),
        )
        .await;
        let workspace = if catalogue {
            configured(&f, &definitions)
        } else {
            index(&f)
        };
        let route = app(&f, &workspace);
        assert_page(
            &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
            &[task.clone()],
            None,
        );
        let mut holder = lock_task(&f, task.task.reference).await;
        let blocker = backend(&mut holder).await;
        expire_soon(&f).await;
        let before = snapshot(&f).await;
        let req = request_with_token("GET", COLLECTION, &f.auth.learner_token, Body::empty());
        let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
        let waited = blocked_by(&f, blocker).await.is_some();
        let expired = waited && wait_expired(&f).await;
        let was_pending = !pending.0.is_finished();
        holder.rollback().await.unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), &mut pending.0)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            waited && expired && was_pending,
            "must observe an owned-row wait and actual PostgreSQL expiry before release"
        );
        assert_eq!(
            response_value(response, StatusCode::UNAUTHORIZED).await,
            json!({"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}})
        );
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        assert_eq!(
            snapshot(&f).await,
            before,
            "expired admitted metadata is not returned or written"
        );
        assert_page(
            &read(&f, &route, &f.auth.manager_token, COLLECTION).await,
            &[],
            None,
        );
        drop((pending, route));
        f.cleanup().await;
    }
}
