use super::fixture::*;
use super::sql_fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_full_definition_and_lookahead_fail_closed_without_filtering(
) {
    let f = Fixture::ready().await;
    let definitions = values();
    let token = &f.auth.learner_token;
    let first = create_at(&f, 10, None, token, manifest("Visible manual row", vec![])).await;
    let target = create_at(
        &f,
        20,
        Some(&definitions[0]),
        token,
        manifest("Catalogue lookahead", vec![]),
    )
    .await;
    let good = configured(&f, &definitions);
    let expected = list(&f, &good, token, None, Some(1)).await.unwrap();
    assert_eq!(expected.entries, vec![entry(&first)]);
    assert_eq!(expected.next_after, Some(first.task.reference));
    let failure = Err(SessionTaskError::Task(TaskStoreError::InvalidRecord));
    assert_eq!(
        list(&f, &index(&f), token, None, Some(1)).await,
        failure,
        "a manual-only index must reject the catalogue lookahead, not skip it"
    );
    assert_eq!(
        list(&f, &configured(&f, &definitions[1..]), token, None, Some(1)).await,
        failure,
        "same package task-name at v2 is not a fallback for v1"
    );
    for axis in ["title", "summary", "evidence", "policy", "unbound"] {
        let mut changed = definitions.clone();
        match axis {
            "title" => changed[0].title.push_str(" changed"),
            "summary" => changed[0].summary.push_str(" changed"),
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
            "{axis} injection must be effective"
        );
        assert_eq!(changed[0].reference, definitions[0].reference);
        assert_eq!(
            list(&f, &configured(&f, &changed), token, None, Some(1)).await,
            failure,
            "{axis}"
        );
    }
    // An unbound definition is still a full catalogue row, not manual work or implicit assessment.
    let unbound = create_at(
        &f,
        30,
        Some(&definitions[1]),
        token,
        manifest("Unbound catalogue", vec![]),
    )
    .await;
    let mut changed = definitions.clone();
    changed[1].evidence_schema = Some(VersionedName {
        name: "sample.document.text".parse().unwrap(),
        version: 2.try_into().unwrap(),
    });
    changed[1].assessment_policy = Some(VersionedName {
        name: "sample.document.review".parse().unwrap(),
        version: 2.try_into().unwrap(),
    });
    assert_ne!(changed[1], definitions[1]);
    assert_eq!(
        list(
            &f,
            &configured(&f, &changed),
            token,
            Some(target.task.reference),
            Some(1)
        )
        .await,
        failure
    );
    assert_eq!(
        list(&f, &good, token, Some(target.task.reference), Some(1))
            .await
            .unwrap()
            .entries,
        vec![entry(&unbound)]
    );
    // Cursor skip semantics remain positional: an invalid row before the cursor is not selected.
    assert_eq!(
        list(&f, &index(&f), token, Some(unbound.task.reference), Some(1))
            .await
            .unwrap()
            .entries,
        vec![]
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_manifest_head_event_and_schema_damage_never_yield_partial_pages(
) {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let first = create_at(&f, 10, None, token, manifest("First valid entry", vec![])).await;
    let target = create_at(&f, 20, None, token, manifest("Selected lookahead", vec![])).await;
    let workspace = index(&f);
    let clean = list(&f, &workspace, token, None, Some(1)).await.unwrap();
    assert_eq!(clean.entries, vec![entry(&first)]);
    assert_eq!(clean.next_after, Some(first.task.reference));
    let task = target.task.reference.task_id.as_uuid();
    let original = query("SELECT snapshot, manifest FROM collaboration_tasks WHERE task_id = $1")
        .bind(task)
        .fetch_one(&f.task_pool)
        .await
        .unwrap();
    let raw: String = original.get("snapshot");
    let metadata: String = original.get("manifest");
    let event = query("SELECT event_id, actor_id, revision, event_type FROM collaboration_task_outbox WHERE task_id = $1")
        .bind(task).fetch_one(&f.task_pool).await.unwrap();
    let event_id: Uuid = event.get("event_id");
    let actor_id: Uuid = event.get("actor_id");
    let revision: i64 = event.get("revision");
    let event_type: String = event.get("event_type");
    let invalid = Err(SessionTaskError::Task(TaskStoreError::InvalidRecord));
    for mode in [
        "head-json",
        "manifest-json",
        "title-agreement",
        "owner",
        "scope",
        "revision",
        "event-json",
        "event-manifest",
        "event-actor",
        "event-revision",
        "event-type",
        "event-id",
        "event-missing",
    ] {
        let statement;
        let mut replacement = raw.clone();
        match mode {
            "head-json" => {
                statement = "UPDATE collaboration_tasks SET snapshot = $2 WHERE task_id = $1";
                replacement = "{}".into();
            }
            "manifest-json" => {
                statement = "UPDATE collaboration_tasks SET manifest = $2 WHERE task_id = $1";
                replacement = "{}".into();
            }
            "event-json" => {
                statement = "UPDATE collaboration_task_outbox SET snapshot = $2 WHERE task_id = $1";
                replacement = "{}".into();
            }
            "event-manifest" => {
                statement = "UPDATE collaboration_task_outbox SET manifest = $2 WHERE task_id = $1";
                replacement = "{}".into();
            }
            "title-agreement" | "owner" | "scope" => {
                statement = "UPDATE collaboration_tasks SET snapshot = $2 WHERE task_id = $1";
                let mut changed = target.task.clone();
                match mode {
                    "title-agreement" => changed.title = "Different legal title".into(),
                    "owner" => changed.owner = f.auth.manager.principal.reference,
                    _ => changed.reference.workspace.workspace_id = id(),
                }
                assert_ne!(changed, target.task);
                replacement = serde_json::to_string(&changed).unwrap();
            }
            _ => statement = "",
        }
        if !statement.is_empty() {
            assert_ne!(
                replacement,
                if mode.contains("manifest") {
                    metadata.clone()
                } else {
                    raw.clone()
                }
            );
            assert_eq!(
                query(statement)
                    .bind(task)
                    .bind(&replacement)
                    .execute(&f.task_pool)
                    .await
                    .unwrap()
                    .rows_affected(),
                1
            );
        } else {
            let statement = match mode {
                "revision" => "UPDATE collaboration_tasks SET revision = revision + 1 WHERE task_id = $1",
                "event-actor" => "UPDATE collaboration_task_outbox SET actor_id = '00000000-0000-0000-0000-000000000000' WHERE task_id = $1",
                "event-revision" => "UPDATE collaboration_task_outbox SET revision = revision + 1 WHERE task_id = $1",
                "event-type" => "UPDATE collaboration_task_outbox SET event_type = 'cyanrex.task.status_changed' WHERE task_id = $1",
                "event-id" => "UPDATE collaboration_task_outbox SET event_id = '00000000-0000-0000-0000-000000000000' WHERE task_id = $1",
                _ => "DELETE FROM collaboration_task_outbox WHERE task_id = $1",
            };
            assert_eq!(
                query(statement)
                    .bind(task)
                    .execute(&f.task_pool)
                    .await
                    .unwrap()
                    .rows_affected(),
                1
            );
        }
        assert_eq!(
            list(&f, &workspace, token, None, Some(1)).await,
            invalid,
            "{mode}"
        );
        assert_eq!(query("UPDATE collaboration_tasks SET revision = $2, snapshot = $3, manifest = $4 WHERE task_id = $1")
            .bind(task).bind(revision).bind(&raw).bind(&metadata).execute(&f.task_pool).await.unwrap().rows_affected(), 1);
        query("DELETE FROM collaboration_task_outbox WHERE task_id = $1")
            .bind(task)
            .execute(&f.task_pool)
            .await
            .unwrap();
        assert_eq!(query("INSERT INTO collaboration_task_outbox (event_id, task_id, revision, actor_id, event_type, snapshot, manifest)
            VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(event_id).bind(task).bind(revision).bind(actor_id).bind(&event_type).bind(&raw).bind(&metadata)
            .execute(&f.task_pool).await.unwrap().rows_affected(), 1);
        assert_eq!(
            list(&f, &workspace, token, None, Some(1)).await.unwrap(),
            clean
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
    assert_eq!(
        list(&f, &workspace, token, None, Some(1)).await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    assert_eq!(
        query("UPDATE collaboration_task_schema SET version = 3")
            .execute(&f.task_pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    let source_workspace =
        SessionTaskContentIndexWorkspace::new(&f.auth.base.schema, scope()).unwrap();
    assert_eq!(
        list(&f, &source_workspace, token, None, None).await,
        Err(SessionTaskError::InvalidNamespace)
    );
    let legacy_workspace =
        SessionTaskContentIndexWorkspace::new(&f.base.task_schema, scope()).unwrap();
    assert_eq!(
        list(&f, &legacy_workspace, token, None, None).await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    assert_eq!(
        list(&f, &workspace, token, None, Some(1)).await.unwrap(),
        clean
    );
    f.cleanup().await;
}
