use super::*;
use index_sql_fixture::create_at;

async fn unavailable(f: &Fixture, route: &Router, path: &str) {
    denied(
        f,
        route,
        request_with_token("GET", path, &f.auth.learner_token, Body::empty()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_full_definitions_heads_manifests_and_lookahead_events_fail_closed() {
    let f = Fixture::ready().await;
    let definitions = values();
    let first = create_at(
        &f,
        10,
        None,
        &f.auth.learner_token,
        manifest("Healthy first", vec![]),
    )
    .await;
    let target = create_at(
        &f,
        20,
        Some(&definitions[0]),
        &f.auth.learner_token,
        manifest("Strict lookahead", vec![]),
    )
    .await;
    let route = app(&f, &configured(&f, &definitions));
    let one = format!("{COLLECTION}?limit=1");
    let healthy = read(&f, &route, &f.auth.learner_token, &one).await;
    assert_page(&healthy, &[first.clone()], Some(first.task.reference));
    unavailable(&f, &app(&f, &index(&f)), &one).await;
    unavailable(&f, &app(&f, &configured(&f, &definitions[1..])), &one).await;
    for axis in ["title", "summary", "evidence", "policy", "unbound"] {
        let mut changed = definitions.clone();
        match axis {
            "title" => changed[0].title.push_str(" drift"),
            "summary" => changed[0].summary.push_str(" drift"),
            "evidence" => {
                changed[0].evidence_schema.as_mut().unwrap().version = 2.try_into().unwrap()
            }
            "policy" => {
                changed[0].assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap()
            }
            _ => {
                changed[0].evidence_schema = None;
                changed[0].assessment_policy = None;
            }
        }
        assert_ne!(
            changed[0], definitions[0],
            "{axis} must change complete metadata"
        );
        assert_eq!(changed[0].reference, definitions[0].reference);
        unavailable(&f, &app(&f, &configured(&f, &changed)), &one).await;
        assert_eq!(read(&f, &route, &f.auth.learner_token, &one).await, healthy);
    }
    let task_id = target.task.reference.task_id.as_uuid();
    let row = query("SELECT snapshot, manifest FROM collaboration_tasks WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(&f.task_pool)
        .await
        .unwrap();
    let raw: String = row.get("snapshot");
    let metadata: String = row.get("manifest");
    let event = query("SELECT event_id, actor_id, revision, event_type FROM collaboration_task_outbox WHERE task_id = $1")
        .bind(task_id).fetch_one(&f.task_pool).await.unwrap();
    let event_id: Uuid = event.get("event_id");
    let actor_id: Uuid = event.get("actor_id");
    let revision: i64 = event.get("revision");
    let event_type: String = event.get("event_type");
    for mode in [
        "head-json",
        "manifest-json",
        "scope",
        "title",
        "head-revision",
        "event-json",
        "event-manifest",
        "event-actor",
        "event-revision",
        "event-type",
        "event-id",
        "event-missing",
    ] {
        let (statement, replacement) = match mode {
            "head-json" => (
                "UPDATE collaboration_tasks SET snapshot = $2 WHERE task_id = $1",
                "{}".into(),
            ),
            "manifest-json" => (
                "UPDATE collaboration_tasks SET manifest = $2 WHERE task_id = $1",
                "{}".into(),
            ),
            "event-json" => (
                "UPDATE collaboration_task_outbox SET snapshot = $2 WHERE task_id = $1",
                "{}".into(),
            ),
            "event-manifest" => (
                "UPDATE collaboration_task_outbox SET manifest = $2 WHERE task_id = $1",
                "{}".into(),
            ),
            "scope" | "title" => {
                let mut changed = target.task.clone();
                if mode == "scope" {
                    changed.reference.workspace.workspace_id = id();
                } else {
                    changed.title.push_str(" legal but divergent");
                }
                assert_ne!(changed, target.task);
                (
                    "UPDATE collaboration_tasks SET snapshot = $2 WHERE task_id = $1",
                    serde_json::to_string(&changed).unwrap(),
                )
            }
            _ => ("", String::new()),
        };
        let affected = if statement.is_empty() {
            let statement = match mode {
                "head-revision" => "UPDATE collaboration_tasks SET revision = revision + 1 WHERE task_id = $1",
                "event-actor" => "UPDATE collaboration_task_outbox SET actor_id = '00000000-0000-0000-0000-000000000000' WHERE task_id = $1",
                "event-revision" => "UPDATE collaboration_task_outbox SET revision = revision + 1 WHERE task_id = $1",
                "event-type" => "UPDATE collaboration_task_outbox SET event_type = 'cyanrex.task.status_changed' WHERE task_id = $1",
                "event-id" => "UPDATE collaboration_task_outbox SET event_id = '00000000-0000-0000-0000-000000000000' WHERE task_id = $1",
                _ => "DELETE FROM collaboration_task_outbox WHERE task_id = $1",
            };
            query(statement)
                .bind(task_id)
                .execute(&f.task_pool)
                .await
                .unwrap()
                .rows_affected()
        } else {
            assert_ne!(
                replacement,
                if mode.contains("manifest") {
                    metadata.clone()
                } else {
                    raw.clone()
                }
            );
            query(statement)
                .bind(task_id)
                .bind(replacement)
                .execute(&f.task_pool)
                .await
                .unwrap()
                .rows_affected()
        };
        assert_eq!(affected, 1, "{mode} must damage the lookahead");
        unavailable(&f, &route, &one).await;
        unavailable(&f, &route, &format!("{COLLECTION}?limit=32")).await;
        assert_eq!(query("UPDATE collaboration_tasks SET revision = $2, snapshot = $3, manifest = $4 WHERE task_id = $1")
            .bind(task_id).bind(revision).bind(&raw).bind(&metadata).execute(&f.task_pool).await.unwrap().rows_affected(), 1);
        query("DELETE FROM collaboration_task_outbox WHERE task_id = $1")
            .bind(task_id)
            .execute(&f.task_pool)
            .await
            .unwrap();
        assert_eq!(query("INSERT INTO collaboration_task_outbox (event_id, task_id, revision, actor_id, event_type, snapshot, manifest)
            VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(event_id).bind(task_id).bind(revision).bind(actor_id).bind(&event_type).bind(&raw).bind(&metadata)
            .execute(&f.task_pool).await.unwrap().rows_affected(), 1);
        assert_eq!(
            read(&f, &route, &f.auth.learner_token, &one).await,
            healthy,
            "{mode} restore control"
        );
    }
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_index_http_empty_namespace_column_damage_is_not_empty_success() {
    let f = Fixture::ready().await;
    let workspace = index(&f);
    let route = app(&f, &workspace);
    assert_page(
        &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
        &[],
        None,
    );
    for (table, column) in [
        ("collaboration_tasks", "task_id"),
        ("collaboration_tasks", "owner_id"),
        ("collaboration_tasks", "revision"),
        ("collaboration_tasks", "snapshot"),
        ("collaboration_tasks", "manifest"),
        ("collaboration_task_outbox", "event_id"),
        ("collaboration_task_outbox", "snapshot"),
        ("collaboration_task_outbox", "manifest"),
    ] {
        query(&format!(
            "ALTER TABLE {table} RENAME COLUMN {column} TO hidden_index_column"
        ))
        .execute(&f.task_pool)
        .await
        .unwrap();
        unavailable(&f, &route, COLLECTION).await;
        query(&format!(
            "ALTER TABLE {table} RENAME COLUMN hidden_index_column TO {column}"
        ))
        .execute(&f.task_pool)
        .await
        .unwrap();
        assert_page(
            &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
            &[],
            None,
        );
    }
    assert_eq!(
        query("UPDATE collaboration_task_schema SET version = 999")
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    unavailable(&f, &route, COLLECTION).await;
    assert_eq!(
        query("UPDATE collaboration_task_schema SET version = 3")
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    let old = SessionTaskContentIndexWorkspace::new(&f.base.task_schema, scope()).unwrap();
    unavailable(&f, &app(&f, &old), COLLECTION).await;
    let source = SessionTaskContentIndexWorkspace::new(&f.auth.base.schema, scope()).unwrap();
    unavailable(&f, &app(&f, &source), COLLECTION).await;
    assert_page(
        &read(&f, &route, &f.auth.learner_token, COLLECTION).await,
        &[],
        None,
    );
    f.assert_pool_restored().await;
    drop(route);
    f.cleanup().await;
}
