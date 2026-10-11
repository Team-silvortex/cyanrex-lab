use super::*;
use index_sql_fixture::{create_at, task_id};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_committed_mixed_metadata_pages_and_positional_cursors() {
    let f = Fixture::ready().await;
    let definitions = values();
    let workspace = configured(&f, &definitions);
    let route = app(&f, &workspace);
    let token = &f.auth.learner_token;
    assert_page(&read(&f, &route, token, COLLECTION).await, &[], None);
    let artifact = f
        .create_artifact(token, "\tPrivate 原文😀\r\n".as_bytes())
        .await;
    let mut tasks = Vec::new();
    for number in (1..=34).rev() {
        let definition = match number % 4 {
            0 => None,
            n => Some(&definitions[n as usize - 1]),
        };
        let refs = if number == 1 {
            vec![artifact.reference.clone()]
        } else {
            vec![]
        };
        tasks.push(
            create_at(
                &f,
                number * 10,
                definition,
                token,
                manifest(&format!("Instance {number}"), refs),
            )
            .await,
        );
    }
    tasks.sort_by_key(|value| value.task.reference.task_id);
    tasks[0] = f
        .auth
        .source
        .transition_session_catalog_content_task(
            token,
            &content(&f, &definitions),
            tasks[0].task.reference.task_id,
            tasks[0].task.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    let first = read(&f, &route, token, COLLECTION).await;
    assert_page(&first, &tasks[..20], Some(tasks[19].task.reference));
    assert_eq!(first["entries"][0]["payload_items"], 1);
    assert_eq!(first["entries"][0]["status"], "ready");
    assert_eq!(first["entries"][0]["revision"], 2);
    assert_eq!(first["entries"][3]["definition_ref"], Value::Null);
    assert!(!first.to_string().contains("Private 原文"));
    let tail = format!("{COLLECTION}?after={}", tasks[19].task.reference.task_id);
    assert_page(&read(&f, &route, token, &tail).await, &tasks[20..], None);
    assert_page(
        &read(&f, &route, token, &format!("{COLLECTION}?limit=32")).await,
        &tasks[..32],
        Some(tasks[31].task.reference),
    );
    let mut after = None;
    let mut collected = Vec::new();
    for expected in &tasks {
        let path = after.map_or_else(
            || format!("{COLLECTION}?limit=1"),
            |cursor: TaskId| format!("{COLLECTION}?limit=1&after={cursor}"),
        );
        let value = read(&f, &route, token, &path).await;
        assert_eq!(value["entries"], json!([projected(expected)]));
        collected.push(value["entries"][0].clone());
        after = if value["next_after"].is_null() {
            None
        } else {
            Some(
                serde_json::from_value::<TaskRef>(value["next_after"].clone())
                    .unwrap()
                    .task_id,
            )
        };
    }
    assert_eq!(collected, tasks.iter().map(projected).collect::<Vec<_>>());
    assert_eq!(after, None);
    let foreign = create_at(
        &f,
        15,
        None,
        &f.auth.manager_token,
        manifest("Foreign cursor", vec![]),
    )
    .await;
    for cursor in [foreign.task.reference.task_id, task_id(19)] {
        assert_page(
            &read(
                &f,
                &route,
                token,
                &format!("{COLLECTION}?after={cursor}&limit=32"),
            )
            .await,
            &tasks[1..33],
            Some(tasks[32].task.reference),
        );
    }
    assert_page(
        &read(
            &f,
            &route,
            token,
            &format!(
                "{COLLECTION}?after={}&limit=32",
                tasks[33].task.reference.task_id
            ),
        )
        .await,
        &[],
        None,
    );
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}
