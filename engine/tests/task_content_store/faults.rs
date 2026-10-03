use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_matching_catalog_definition_tampering_cannot_enter_manual_storage() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    let versioned = |name: &str| VersionedName {
        name: name.parse().unwrap(),
        version: rev(1),
    };
    let definition = TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: versioned("example.documents"),
            name: "example.documents.summary".parse().unwrap(),
            version: rev(1),
        },
        title: "A catalogue task".into(),
        summary: "Explicit typed work".into(),
        evidence_schema: versioned("example.documents.text"),
        assessment_policy: versioned("example.documents.minimum_words"),
    };
    let mut tampered = original.task.clone();
    tampered.definition = Some(definition);
    let raw = serde_json::to_string(&tampered).unwrap();
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        query(&format!("UPDATE {table} SET snapshot = $1"))
            .bind(&raw)
            .execute(&f.pool)
            .await
            .unwrap();
    }
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_installs_only_explicitly_empty_namespaces() {
    let f = Fixture::new().await;
    assert!(f.store.get(reference(), owner()).await.is_err());
    let objects: i64 = query(
        "SELECT count(*) AS n FROM pg_class WHERE relnamespace = current_schema()::regnamespace",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(objects, 0);
    f.sql("CREATE TABLE unrelated (value TEXT)").await;
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    f.sql("DROP TABLE unrelated").await;
    f.store.install_empty_namespace().await.unwrap();
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    f.sql("UPDATE collaboration_task_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_row_event_and_manifest_tampering_fail_closed() {
    for statement in [
        "UPDATE collaboration_tasks SET manifest = '{}'",
        "UPDATE collaboration_task_outbox SET manifest = '{}'",
        "UPDATE collaboration_tasks SET manifest = jsonb_set(manifest::jsonb, '{title}', '\"different\"')::text",
        "UPDATE collaboration_task_outbox SET manifest = jsonb_set(manifest::jsonb, '{title}', '\"different\"')::text",
        "UPDATE collaboration_tasks SET snapshot = jsonb_set(snapshot::jsonb, '{title}', '\"different\"')::text",
        "UPDATE collaboration_tasks SET manifest = jsonb_set(manifest::jsonb, '{payload}', (manifest::jsonb->'payload') || (manifest::jsonb->'payload'))::text",
        "UPDATE collaboration_task_outbox SET actor_id = '267c6b0f-182c-4980-a925-cdc967327c57'",
        "UPDATE collaboration_task_outbox SET event_type = 'cyanrex.task.content_updated'",
        "DELETE FROM collaboration_task_outbox",
    ] {
        let f = Fixture::ready().await;
        f.create().await;
        f.sql(statement).await;
        let events = f.count("collaboration_task_outbox").await;
        assert_eq!(f.store.get(reference(), owner()).await, Err(TaskStoreError::InvalidRecord), "{statement}");
        assert_eq!(f.store.replace(reference(), owner(), rev(1), content("Edited", vec![])).await,
            Err(TaskStoreError::InvalidRecord), "{statement}");
        assert_eq!(f.store.transition(reference(), owner(), rev(1), TaskStatus::Ready).await,
            Err(TaskStoreError::InvalidRecord), "{statement}");
        assert_eq!(f.count("collaboration_task_outbox").await, events);
        let other = PrincipalRef { principal_id: id(), ..owner() };
        assert_eq!(f.store.get(reference(), other).await.unwrap(), None);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_oversized_persisted_json_is_rejected_at_both_record_boundaries() {
    for (table, column, limit) in [
        ("collaboration_tasks", "manifest", 65536),
        ("collaboration_task_outbox", "manifest", 65536),
        ("collaboration_tasks", "snapshot", 32768),
        ("collaboration_task_outbox", "snapshot", 32768),
    ] {
        let f = Fixture::ready().await;
        f.create().await;
        // Simulate external corruption after removing only this synthetic byte constraint.
        let constraints = query(
            "SELECT conname FROM pg_constraint WHERE conrelid = $1::regclass
            AND contype = 'c' AND pg_get_constraintdef(oid) LIKE $2",
        )
        .bind(format!("{}.{}", f.schema, table))
        .bind(format!("%octet_length({column})%"))
        .fetch_all(&f.pool)
        .await
        .unwrap();
        assert_eq!(
            constraints.len(),
            1,
            "fixture must remove the intended byte constraint"
        );
        let name: String = constraints[0].get("conname");
        f.sql(&format!("ALTER TABLE {table} DROP CONSTRAINT \"{name}\""))
            .await;
        query(&format!("UPDATE {table} SET {column} = $1"))
            .bind(" ".repeat(limit + 1))
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(
            f.store.get(reference(), owner()).await,
            Err(TaskStoreError::InvalidRecord)
        );
        assert_eq!(
            f.store
                .replace(reference(), owner(), rev(1), manifest())
                .await,
            Err(TaskStoreError::InvalidRecord)
        );
        assert_eq!(f.count("collaboration_task_outbox").await, 1);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_missing_nullable_or_rls_storage_never_returns_partial_data() {
    for statement in [
        "ALTER TABLE collaboration_tasks DROP COLUMN manifest",
        "ALTER TABLE collaboration_task_outbox DROP COLUMN manifest",
        "ALTER TABLE collaboration_tasks ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE collaboration_task_outbox ENABLE ROW LEVEL SECURITY",
    ] {
        let f = Fixture::ready().await;
        f.create().await;
        f.sql(statement).await;
        assert!(f.store.get(reference(), owner()).await.is_err());
        assert!(f
            .store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await
            .is_err());
        assert_eq!(f.count("collaboration_task_outbox").await, 1);
        f.cleanup().await;
    }
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let f = Fixture::ready().await;
        f.create().await;
        f.sql(&format!(
            "ALTER TABLE {table} ALTER COLUMN manifest DROP NOT NULL"
        ))
        .await;
        f.sql(&format!("UPDATE {table} SET manifest = NULL")).await;
        assert!(f.store.get(reference(), owner()).await.is_err());
        assert!(f
            .store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await
            .is_err());
        assert_eq!(f.count("collaboration_task_outbox").await, 1);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_suppressed_rows_or_events_roll_back_the_complete_edit() {
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let f = Fixture::ready().await;
        f.sql("CREATE FUNCTION suppress_content() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
        let trigger = format!("CREATE TRIGGER suppress_content BEFORE INSERT OR UPDATE ON {table} FOR EACH ROW EXECUTE FUNCTION suppress_content()");
        f.sql(&trigger).await;
        assert!(f
            .store
            .create(reference().task_id, owner(), manifest())
            .await
            .is_err());
        assert_eq!(f.count("collaboration_tasks").await, 0);
        assert_eq!(f.count("collaboration_task_outbox").await, 0);
        f.sql(&format!("DROP TRIGGER suppress_content ON {table}"))
            .await;
        let original = f.create().await;
        f.sql(&trigger).await;
        assert!(f
            .store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await
            .is_err());
        assert!(f
            .store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await
            .is_err());
        f.unchanged(&original, 1).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_consistent_trigger_tampering_is_not_the_requested_write() {
    let f = Fixture::ready().await;
    f.sql("CREATE SEQUENCE content_fault_calls").await;
    f.sql("CREATE FUNCTION alter_content() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('content_fault_calls');
        NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{title}', '\"Tampered consistently\"')::text;
        NEW.manifest = jsonb_set(NEW.manifest::jsonb, '{title}', '\"Tampered consistently\"')::text;
        UPDATE collaboration_tasks SET snapshot = NEW.snapshot, manifest = NEW.manifest WHERE task_id = NEW.task_id;
        RETURN NEW; END $$").await;
    let trigger = "CREATE TRIGGER alter_content BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION alter_content()";
    f.sql(trigger).await;
    assert_eq!(
        f.store
            .create(reference().task_id, owner(), manifest())
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    f.sql("DROP TRIGGER alter_content ON collaboration_task_outbox")
        .await;
    let original = f.create().await;
    f.sql(trigger).await;
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    let calls: i64 = query("SELECT last_value FROM content_fault_calls")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("last_value");
    assert_eq!(
        calls, 3,
        "each mutation must reach the intended post-write fault"
    );
    f.unchanged(&original, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_deferred_commit_failure_returns_no_success_or_partial_event() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    f.sql("CREATE FUNCTION fail_content_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic content commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_content_commit AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_content_commit()").await;
    assert_eq!(
        f.store.create(id(), owner(), manifest()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(f.count("collaboration_tasks").await, 1);
    f.unchanged(&original, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_post_write_scope_or_version_drift_rolls_back_every_record() {
    for mutation in [
        "version = 99",
        "workspace_id = '267c6b0f-182c-4980-a925-cdc967327c57'",
    ] {
        let f = Fixture::ready().await;
        let original = f.create().await;
        f.sql(&format!(
            "CREATE FUNCTION change_content_scope() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            UPDATE collaboration_task_schema SET {mutation}; RETURN NEW; END $$"
        ))
        .await;
        f.sql("CREATE TRIGGER change_content_scope BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION change_content_scope()").await;
        assert!(f
            .store
            .replace(reference(), owner(), rev(1), content("Edited", vec![]))
            .await
            .is_err());
        assert!(f
            .store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await
            .is_err());
        f.unchanged(&original, 1).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_revision_overflow_never_wraps_or_appends_an_event() {
    let f = Fixture::ready().await;
    f.create().await;
    f.sql(
        "UPDATE collaboration_tasks SET revision = 9007199254740991,
        snapshot = jsonb_set(snapshot::jsonb, '{revision}', '9007199254740991')::text",
    )
    .await;
    f.sql("UPDATE collaboration_task_outbox SET revision = 9007199254740991,
        snapshot = (SELECT snapshot FROM collaboration_tasks), event_type = 'cyanrex.task.content_updated'").await;
    let original = f.store.get(reference(), owner()).await.unwrap().unwrap();
    assert_eq!(
        f.store
            .replace(
                reference(),
                owner(),
                rev(9_007_199_254_740_991),
                content("Edited", vec![])
            )
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .transition(
                reference(),
                owner(),
                rev(9_007_199_254_740_991),
                TaskStatus::Ready
            )
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    f.unchanged(&original, 1).await;
    f.cleanup().await;
}
