use super::fixture::*;
use super::sql_fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_empty_default_and_bounded_uuid_pages_are_read_only() {
    let f = Fixture::ready().await;
    let workspace = index(&f);
    let token = &f.auth.learner_token;
    let empty = list(&f, &workspace, token, None, None).await.unwrap();
    assert!(empty.entries.is_empty());
    assert_eq!(empty.next_after, None);
    let mut tasks = Vec::new();
    // Insert backwards: neither physical insertion order nor identical timestamps define pages.
    for number in (1..=35).rev() {
        tasks.push(
            create_at(
                &f,
                number,
                None,
                token,
                manifest(&format!("Task {number}"), vec![]),
            )
            .await,
        );
    }
    tasks.sort_by_key(|task| task.task.reference.task_id);
    let first = list(&f, &workspace, token, None, None).await.unwrap();
    assert_eq!(
        first.entries,
        tasks[..20].iter().map(entry).collect::<Vec<_>>()
    );
    assert_eq!(first.next_after, Some(tasks[19].task.reference));
    let last = list(&f, &workspace, token, first.next_after, None)
        .await
        .unwrap();
    assert_eq!(
        last.entries,
        tasks[20..].iter().map(entry).collect::<Vec<_>>()
    );
    assert_eq!(last.next_after, None);
    let large = list(&f, &workspace, token, None, Some(32)).await.unwrap();
    assert_eq!(
        large.entries,
        tasks[..32].iter().map(entry).collect::<Vec<_>>()
    );
    assert_eq!(large.next_after, Some(tasks[31].task.reference));
    let exact = list(
        &f,
        &workspace,
        token,
        Some(tasks[2].task.reference),
        Some(32),
    )
    .await
    .unwrap();
    assert_eq!(
        exact.entries,
        tasks[3..].iter().map(entry).collect::<Vec<_>>()
    );
    assert_eq!(
        exact.next_after, None,
        "exactly limit rows is not evidence of another page"
    );
    let none = list(
        &f,
        &workspace,
        token,
        Some(tasks[34].task.reference),
        Some(1),
    )
    .await
    .unwrap();
    assert!(none.entries.is_empty());
    assert_eq!(none.next_after, None);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_mixed_definitions_owner_cursor_and_namespace_isolation() {
    let f = Fixture::ready().await;
    let definitions = values();
    let workspace = configured(&f, &definitions);
    let token = &f.auth.learner_token;
    let artifact = f
        .create_artifact(token, b"Private payload is not an index field")
        .await;
    let mut tasks = vec![create_at(&f, 10, None, token, manifest("Manual instance", vec![])).await];
    for (number, definition) in [
        (30, &definitions[0]),
        (50, &definitions[1]),
        (70, &definitions[2]),
    ] {
        tasks.push(
            create_at(
                &f,
                number,
                Some(definition),
                token,
                manifest(
                    "Instance title, not definition title",
                    vec![artifact.reference.clone()],
                ),
            )
            .await,
        );
    }
    let ready = f
        .auth
        .source
        .transition_session_catalog_content_task(
            token,
            &content(&f, &definitions),
            tasks[1].task.reference.task_id,
            tasks[1].task.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    tasks[1] = ready;
    let foreign = create_at(
        &f,
        20,
        None,
        &f.auth.manager_token,
        manifest("Other person's private task", vec![]),
    )
    .await;
    let page = list(&f, &workspace, token, None, Some(32)).await.unwrap();
    assert_eq!(page.entries, tasks.iter().map(entry).collect::<Vec<_>>());
    assert_eq!(page.next_after, None);
    assert_eq!(
        page.entries[1].definition_ref,
        Some(definitions[0].reference.clone())
    );
    assert_eq!(
        page.entries[2].definition_ref,
        Some(definitions[1].reference.clone())
    );
    assert_eq!(
        page.entries[3].definition_ref,
        Some(definitions[2].reference.clone())
    );
    assert_eq!(page.entries[1].revision, tasks[1].task.revision);
    assert_eq!(page.entries[1].status, TaskStatus::Ready);
    assert_eq!(page.entries[0].payload_items, 0);
    assert_eq!(page.entries[1].payload_items, 1);
    // A position is neither an ownership claim nor a request to read that cursor's row.
    for cursor in [
        foreign.task.reference,
        TaskRef {
            workspace: scope(),
            task_id: task_id(25),
        },
    ] {
        assert_eq!(
            list(&f, &workspace, token, Some(cursor), Some(32))
                .await
                .unwrap()
                .entries,
            tasks[1..].iter().map(entry).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        list(&f, &workspace, &f.auth.manager_token, None, None)
            .await
            .unwrap()
            .entries,
        vec![entry(&foreign)]
    );
    // Even a malformed other-owner record must not be decoded or exposed by this owner's page.
    assert_eq!(
        query("UPDATE collaboration_tasks SET snapshot = '{}' WHERE task_id = $1")
            .bind(foreign.task.reference.task_id.as_uuid())
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        list(&f, &workspace, token, None, None)
            .await
            .unwrap()
            .entries,
        tasks.iter().map(entry).collect::<Vec<_>>()
    );

    let other_schema = format!("index_other_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {other_schema}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let other_pool = input_fixture::pool_for(&other_schema).await;
    let store = TaskContentStore::new(other_pool.clone(), scope());
    store.install_empty_namespace().await.unwrap();
    let content_workspace =
        SessionTaskContentWorkspace::new(&other_schema, scope(), f.artifact_workspace.clone())
            .unwrap();
    let other_task = f
        .auth
        .source
        .create_session_content_task(
            token,
            &content_workspace,
            task_id(80),
            manifest("Separate namespace", vec![]),
        )
        .await
        .unwrap();
    let other_index = SessionTaskContentIndexWorkspace::new(&other_schema, scope()).unwrap();
    let before = snapshot_with(&f, &[&other_schema]).await;
    assert_eq!(
        f.auth
            .source
            .list_session_content_tasks(token, &other_index, None, None)
            .await
            .unwrap()
            .entries,
        vec![entry(&other_task)]
    );
    assert_eq!(
        f.auth
            .source
            .list_session_content_tasks(token, &workspace, None, None)
            .await
            .unwrap()
            .entries,
        tasks.iter().map(entry).collect::<Vec<_>>()
    );
    assert_eq!(snapshot_with(&f, &[&other_schema]).await, before);
    other_pool.close().await;
    query(&format!("DROP SCHEMA {other_schema} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    drop((store, content_workspace, other_index));
    f.cleanup().await;
}
