use super::*;
use cyanrex_engine::services::task_store::{TaskContentIndexPage, TaskStore};
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_content_index_orders_bounds_and_projects_mixed_tasks() {
    let f = Fixture::ready().await;
    let mut expected = Vec::new();
    for value in (1..=34).rev() {
        let definition = match value % 4 {
            0 => None,
            index => Some(index as usize - 1),
        };
        let mut saved = f.create(value, owner(), definition).await;
        if value == 1 {
            saved = f
                .catalog
                .transition(saved.task.reference, owner(), rev(1), TaskStatus::Ready)
                .await
                .unwrap();
        }
        expected.push(projected(&saved));
    }
    expected.sort_by_key(|entry| entry.reference.task_id);
    let before = f.rows().await;
    let index = f.index.clone();
    let page = index.list(owner(), None, None).await.unwrap();
    assert_eq!(
        page,
        TaskContentIndexPage {
            entries: expected[..20].to_vec(),
            next_after: Some(task(20))
        }
    );
    let tail = index
        .list(owner(), page.next_after, Some(32))
        .await
        .unwrap();
    assert_eq!(tail.entries, expected[20..]);
    assert_eq!(tail.next_after, None);
    let max = index.list(owner(), None, Some(32)).await.unwrap();
    assert_eq!(max.entries, expected[..32]);
    assert_eq!(max.next_after, Some(task(32)));
    assert_eq!(
        index
            .list(owner(), max.next_after, Some(32))
            .await
            .unwrap()
            .entries,
        expected[32..]
    );
    let mut collected = Vec::new();
    let mut after = None;
    let mut ended = false;
    for _ in 0..=expected.len() {
        let page = index.list(owner(), after, Some(1)).await.unwrap();
        assert_eq!(page.entries.len(), 1);
        collected.extend(page.entries);
        match page.next_after {
            Some(cursor) => {
                assert!(after.is_none_or(|previous| cursor.task_id > previous.task_id));
                after = Some(cursor);
            }
            None => {
                ended = true;
                break;
            }
        }
    }
    assert!(
        ended,
        "one-entry pages must terminate within the fixed fixture bound"
    );
    assert_eq!(collected, expected);
    for after in [task(34), task(900)] {
        assert_eq!(
            index.list(owner(), Some(after), None).await.unwrap(),
            TaskContentIndexPage {
                entries: vec![],
                next_after: None
            }
        );
    }
    // The neutral index changes neither old writable mode policy nor catalogue admission.
    assert_eq!(
        f.manual.get(task(1), owner()).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.catalog.get(task(4), owner()).await,
        Err(TaskStoreError::InvalidRecord)
    );
    let manual_only = TaskContentIndexStore::new(f.pool.clone(), scope());
    assert_eq!(
        manual_only.list(owner(), None, Some(32)).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert!(expected.iter().any(|entry| entry.definition_ref.is_none()));
    for definition in &f.definitions {
        assert!(expected
            .iter()
            .any(|entry| entry.definition_ref.as_ref() == Some(&definition.reference)));
    }
    assert_eq!(f.rows().await, before);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_content_index_rejects_corrupt_selected_and_lookahead_rows() {
    let f = Fixture::ready().await;
    f.create(1, owner(), None).await;
    let bound = f.create(2, owner(), Some(0)).await;
    let unbound = f.create(3, owner(), Some(1)).await;
    assert_eq!(
        f.index
            .list(owner(), None, Some(1))
            .await
            .unwrap()
            .next_after,
        Some(task(1))
    );
    assert_eq!(
        f.index
            .list(owner(), None, Some(32))
            .await
            .unwrap()
            .entries
            .len(),
        3
    );
    for axis in 0..8 {
        let mut bad = bound.task.clone();
        match axis {
            0 => bad.definition.as_mut().unwrap().reference.version = rev(99),
            1 => bad.definition.as_mut().unwrap().reference.package.version = rev(99),
            2 => bad
                .definition
                .as_mut()
                .unwrap()
                .summary
                .push_str(" changed"),
            3 => {
                let definition = bad.definition.as_mut().unwrap();
                definition.evidence_schema = None;
                definition.assessment_policy = None;
            }
            4 => bad.owner = other_owner(),
            5 => {
                bad.reference.workspace.workspace_id =
                    other_owner().principal_id.as_uuid().try_into().unwrap()
            }
            6 => bad.input_refs.clear(),
            7 => bad.status = TaskStatus::Ready,
            _ => unreachable!(),
        }
        assert_ne!(bad, bound.task, "fault {axis} must change the record");
        f.set_snapshot(task(2), &bad).await;
        f.reject_unchanged(None, Some(1)).await; // The healthy first row must not mask bad lookahead.
        f.reject_unchanged(None, Some(32)).await;
        let after = f
            .index
            .list(owner(), Some(task(2)), Some(32))
            .await
            .unwrap();
        assert_eq!(after.entries, vec![projected(&unbound)]);
        f.restore(&bound).await;
    }
    let mut rebound = unbound.task.clone();
    let definition = rebound.definition.as_mut().unwrap();
    definition.evidence_schema = Some(VersionedName {
        name: "sample.documents.text".parse().unwrap(),
        version: rev(2),
    });
    definition.assessment_policy = Some(VersionedName {
        name: "sample.documents.editorial".parse().unwrap(),
        version: rev(2),
    });
    assert_ne!(rebound, unbound.task);
    assert_eq!(
        serde_json::from_str::<TaskSnapshot>(&serde_json::to_string(&rebound).unwrap()).unwrap(),
        rebound
    );
    f.set_snapshot(task(3), &rebound).await;
    f.reject_unchanged(Some(task(1)), Some(1)).await;
    f.restore(&unbound).await;
    for column in ["snapshot", "manifest"] {
        f.set_column(task(2), column, "{}").await;
        f.reject_unchanged(None, Some(1)).await;
        f.restore(&bound).await;
    }
    for (column, raw) in [
        ("snapshot", "{}"),
        ("manifest", "{}"),
        ("event_type", "cyanrex.task.content_updated"),
    ] {
        assert_eq!(
            query(&format!(
                "UPDATE collaboration_task_outbox SET {column} = $1 WHERE task_id = $2"
            ))
            .bind(raw)
            .bind(task(2).task_id.as_uuid())
            .execute(&f.pool)
            .await
            .unwrap()
            .rows_affected(),
            1
        );
        f.reject_unchanged(None, Some(1)).await;
        if column == "event_type" {
            f.sql("UPDATE collaboration_task_outbox SET event_type = 'cyanrex.task.created'")
                .await;
        } else {
            f.restore(&bound).await;
        }
    }
    // The selected raw projection must also reject an externally enlarged, otherwise valid JSON.
    let constraints = query("SELECT conname FROM pg_constraint WHERE conrelid = 'collaboration_tasks'::regclass AND contype = 'c' AND pg_get_constraintdef(oid) LIKE '%octet_length(snapshot)%'")
        .fetch_all(&f.pool).await.unwrap();
    assert_eq!(constraints.len(), 1);
    let constraint: String = constraints[0].get("conname");
    f.sql(&format!(
        "ALTER TABLE collaboration_tasks DROP CONSTRAINT \"{constraint}\""
    ))
    .await;
    let raw = format!(
        "{}{}",
        serde_json::to_string(&bound.task).unwrap(),
        " ".repeat(32769)
    );
    assert!(raw.len() > 32768);
    assert_eq!(
        serde_json::from_str::<TaskSnapshot>(&raw).unwrap(),
        bound.task
    );
    query("UPDATE collaboration_tasks SET snapshot = $1 WHERE task_id = $2")
        .bind(raw)
        .bind(task(2).task_id.as_uuid())
        .execute(&f.pool)
        .await
        .unwrap();
    f.reject_unchanged(None, Some(1)).await;
    f.restore(&bound).await;
    assert_eq!(
        query("DELETE FROM collaboration_task_outbox WHERE task_id = $1")
            .bind(task(2).task_id.as_uuid())
            .execute(&f.pool)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    f.reject_unchanged(None, Some(1)).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_content_index_owner_cursor_scope_and_schema_are_fail_closed() {
    let f = Fixture::ready().await;
    let first = f.create(2, owner(), None).await;
    let last = f.create(4, owner(), None).await;
    f.create(3, other_owner(), None).await;
    let before = f.rows().await;
    let manual = TaskContentIndexStore::new(f.pool.clone(), scope());
    assert_eq!(
        manual.list(owner(), None, None).await.unwrap().entries,
        vec![projected(&first), projected(&last)]
    );
    let absent = PrincipalRef {
        principal_id: task(900).task_id.as_uuid().try_into().unwrap(),
        ..owner()
    };
    assert!(manual
        .list(absent, None, None)
        .await
        .unwrap()
        .entries
        .is_empty());
    // A foreign-owner cursor is only a UUID position, never a detached owner/existence assertion.
    let after_foreign = manual.list(owner(), Some(task(3)), Some(32)).await.unwrap();
    assert_eq!(after_foreign.entries, vec![projected(&last)]);
    assert_eq!(after_foreign.next_after, None);
    assert_eq!(f.rows().await, before);
    f.set_column(task(3), "snapshot", "{}").await;
    let corrupted = f.rows().await;
    assert_eq!(
        manual.list(owner(), Some(task(3)), None).await.unwrap(),
        after_foreign
    );
    assert_eq!(
        manual.list(other_owner(), None, None).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(f.rows().await, corrupted);
    let foreign_owner = PrincipalRef {
        authority_id: task(901).task_id.as_uuid().try_into().unwrap(),
        ..owner()
    };
    assert_eq!(
        manual.list(foreign_owner, None, None).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let foreign_cursor = TaskRef {
        workspace: WorkspaceRef {
            workspace_id: task(902).task_id.as_uuid().try_into().unwrap(),
            ..scope()
        },
        ..task(2)
    };
    assert_eq!(
        manual.list(owner(), Some(foreign_cursor), None).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(f.rows().await, corrupted);
    f.cleanup().await;
    for (statement, expected) in [
        ("UPDATE collaboration_task_schema SET version = 2", TaskStoreError::UnsupportedSchema),
        ("UPDATE collaboration_task_schema SET workspace_id = '00000000-0000-0000-0000-000000000999'", TaskStoreError::ScopeMismatch),
        ("ALTER TABLE collaboration_tasks ENABLE ROW LEVEL SECURITY", TaskStoreError::UnsupportedSchema),
        ("ALTER TABLE collaboration_tasks DROP COLUMN manifest", TaskStoreError::StorageUnavailable),
        ("ALTER TABLE collaboration_task_outbox DROP COLUMN manifest", TaskStoreError::StorageUnavailable),
    ] {
        let f = Fixture::ready().await;
        f.create(1, owner(), None).await;
        let healthy = f.rows().await;
        assert_eq!(
            f.index.list(absent, None, None).await.unwrap(),
            TaskContentIndexPage { entries: vec![], next_after: None },
            "the empty owner's healthy schema must succeed before {statement}"
        );
        assert_eq!(f.rows().await, healthy);
        f.sql(statement).await;
        let before = f.rows().await;
        assert_eq!(f.index.list(owner(), None, None).await, Err(expected), "{statement}");
        assert_eq!(
            f.index.list(absent, None, None).await,
            Err(expected),
            "an empty owner must not hide structural damage: {statement}"
        );
        assert_eq!(f.rows().await, before);
        f.cleanup().await;
    }
    let f = Fixture::new().await;
    TaskStore::new(f.pool.clone(), scope())
        .install_empty_namespace()
        .await
        .unwrap();
    let before = f.rows().await;
    assert_eq!(
        f.index.list(owner(), None, None).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(f.rows().await, before);
    f.cleanup().await;
    let f = Fixture::new().await;
    assert!(f.index.list(owner(), None, None).await.is_err());
    let objects: i64 = query(
        "SELECT count(*) AS n FROM pg_class WHERE relnamespace = current_schema()::regnamespace",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(objects, 0, "reading must not install or fall back");
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_content_index_cancelled_table_wait_releases_resources_without_writes() {
    let f = Fixture::ready().await;
    let saved = f.create(1, owner(), None).await;
    let before = f.rows().await;
    let mut blocker = f.pool.begin().await.unwrap();
    let pid: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *blocker)
        .await
        .unwrap()
        .get("pid");
    query("LOCK TABLE collaboration_tasks IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let index = f.index.clone();
    let pending = tokio::spawn(async move { index.list(owner(), None, None).await });
    f.wait_blocked(pid).await;
    assert!(!pending.is_finished());
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    f.assert_pool_restored().await;
    assert_eq!(f.rows().await, before);
    assert_eq!(
        f.index.list(owner(), None, None).await.unwrap().entries,
        vec![projected(&saved)]
    );
    f.cleanup().await;
}
