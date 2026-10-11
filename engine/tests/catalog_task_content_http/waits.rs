use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_expiry_at_observed_read_write_wait_and_post_write_blob_loss_fail_closed(
) {
    for reading in [false, true] {
        let (f, workspace, route, task, old, new) = pair().await;
        let before = observed(&f).await;
        let mut holder = if reading {
            let mut tx = f.artifact_pool.begin().await.unwrap();
            query(
                "SELECT artifact_id FROM collaboration_artifacts WHERE artifact_id = $1 FOR UPDATE",
            )
            .bind(old.reference.artifact_id.as_uuid())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
            tx
        } else {
            f.pause_task_event().await
        };
        let blocker = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1").bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let path = task_path(task.task.reference.task_id);
        let req = if reading {
            request("GET", &path, &f.auth.learner_token, Body::empty())
        } else {
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(&task, &manifest("No expired commit", vec![new.reference])),
            )
        };
        let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
        blocked_by(&f, blocker).await;
        tokio::time::timeout(Duration::from_secs(2),async {
            loop {
                let expired: bool = query(&format!("SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions WHERE token = $1",f.auth.base.schema)).bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
                if expired { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("synthetic Session did not expire at the observed wait");
        assert!(!pending.0.is_finished());
        holder.rollback().await.unwrap();
        finish(&mut pending, StatusCode::UNAUTHORIZED, "invalid_session").await;
        assert_eq!(observed(&f).await, before);
        f.assert_pool_restored().await;
        drop((pending, route, workspace));
        f.cleanup().await;
    }
    let (f, workspace, route, task, old, new) = pair().await;
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let req = request(
        "PUT",
        &format!("{}/content", task_path(task.task.reference.task_id)),
        &f.auth.learner_token,
        replace_body(
            &task,
            &manifest(
                "Cannot erase old bytes after writes",
                vec![new.reference.clone()],
            ),
        ),
    );
    let mut pending = Pending(tokio::spawn(route.clone().oneshot(req)));
    blocked_by(&f, blocker).await;
    fs::remove_file(f.blob(&old.reference)).unwrap();
    // The external corruption is intentional. Only the rejected operation must leave the now
    // observed seven-table/file state unchanged; HTTP outcome is still not a rollback receipt.
    let before = observed(&f).await;
    holder.rollback().await.unwrap();
    finish(
        &mut pending,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
    )
    .await;
    assert_eq!(observed(&f).await, before);
    assert_eq!(
        fs::read(f.blob(&new.reference)).unwrap(),
        b"Replacement input"
    );
    f.assert_pool_restored().await;
    drop((pending, route, workspace));
    f.cleanup().await;
}
