use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_installation_and_schema_incarnation_are_not_adopted() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, &[]);
    attempt
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await
        .unwrap();
    let duplicate_install = f.journal.install_empty_namespace().await;
    let wrong = DraftIntentJournal::new(f.journal_pool.clone(), scope(), Uuid::new_v4()).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, wrong, f.workspace.clone()).unwrap();
    let wrong_id = f
        .auth
        .source
        .get_session_draft_intent(
            &f.auth.learner_token,
            &workspace,
            attempt.task_ref().task_id,
        )
        .await
        .err();
    let initial = f.metadata().await;
    let mut outcomes = Vec::new();
    for (assignment, expected) in [
        (
            "version = 99".into(),
            DraftIntentJournalError::UnsupportedSchema,
        ),
        (
            format!("installation_id = '{}'", Uuid::new_v4()),
            DraftIntentJournalError::IncarnationMismatch,
        ),
        (
            format!("workspace_id = '{}'", Uuid::new_v4()),
            DraftIntentJournalError::ScopeMismatch,
        ),
        (
            format!("authority_id = '{}'", Uuid::new_v4()),
            DraftIntentJournalError::ScopeMismatch,
        ),
    ] {
        let mut transaction = f.journal_pool.begin().await.unwrap();
        query(&format!(
            "UPDATE collaboration_draft_intent_schema SET {assignment}"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        let before = f.metadata().await;
        let error = f
            .get(&f.auth.learner_token, attempt.task_ref().task_id)
            .await
            .err();
        outcomes.push((error, expected, before == f.metadata().await));
        f.sql(&format!("UPDATE collaboration_draft_intent_schema SET version = 1, installation_id = '{}', workspace_id = '{}', authority_id = '{}'",
            f.installation, scope().workspace_id, scope().authority_id)).await;
    }
    // Locate the actual key structurally; a fixed migration constraint name is not the contract.
    let primary_key: String = query("SELECT conname FROM pg_constraint WHERE conrelid = 'collaboration_draft_intents'::regclass AND contype = 'p'")
        .fetch_one(&f.journal_pool).await.unwrap().get("conname");
    let mut shape_outcomes = Vec::new();
    for (damage, restore) in [
        (
            format!("ALTER TABLE collaboration_draft_intents DROP CONSTRAINT \"{}\"", primary_key.replace('"', "\"\"")),
            "ALTER TABLE collaboration_draft_intents ADD CONSTRAINT intent_renamed_primary_key PRIMARY KEY (task_id)".to_owned(),
        ),
        (
            "ALTER TABLE collaboration_draft_intents ALTER COLUMN account_id DROP NOT NULL".to_owned(),
            "ALTER TABLE collaboration_draft_intents ALTER COLUMN account_id SET NOT NULL".to_owned(),
        ),
        (
            "ALTER TABLE collaboration_draft_intents ALTER COLUMN source_namespace TYPE varchar".to_owned(),
            "ALTER TABLE collaboration_draft_intents ALTER COLUMN source_namespace TYPE text".to_owned(),
        ),
    ] {
        let fresh = prepare(&f, &[]);
        f.sql(&damage).await;
        let before = f.state().await;
        let read = f.get(&f.auth.learner_token, attempt.task_ref().task_id).await.err();
        let write = fresh.register_intent(&f.auth.learner_token, &f.intent_workspace).await.err();
        shape_outcomes.push((read, write, before == f.state().await));
        f.sql(&restore).await;
    }
    let renamed_key_still_reads = f
        .get(&f.auth.learner_token, attempt.task_ref().task_id)
        .await
        .is_ok_and(|value| value.is_some());
    let occupied_name = format!("intent_occupied_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {occupied_name}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    query(&format!(
        "CREATE TABLE {occupied_name}.unrelated (id integer)"
    ))
    .execute(&f.auth.base.admin)
    .await
    .unwrap();
    let pool = input_fixture::pool_for(&occupied_name).await;
    let occupied = DraftIntentJournal::new(pool.clone(), scope(), Uuid::new_v4())
        .unwrap()
        .install_empty_namespace()
        .await;
    pool.close().await;
    query(&format!("DROP SCHEMA {occupied_name} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let unchanged = initial == f.metadata().await;
    let rows = f.count().await;
    f.cleanup().await;
    assert_eq!(
        duplicate_install,
        Err(DraftIntentJournalError::NamespaceNotEmpty)
    );
    assert_eq!(occupied, Err(DraftIntentJournalError::NamespaceNotEmpty));
    assert_eq!(
        wrong_id,
        Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::IncarnationMismatch
        ))
    );
    for (actual, expected, intact) in outcomes {
        assert_eq!(actual, Some(SessionDraftIntentError::Journal(expected)));
        assert!(intact);
    }
    for (read, write, intact) in shape_outcomes {
        let expected = Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::UnsupportedSchema,
        ));
        assert_eq!(read, expected);
        assert_eq!(write, expected);
        assert!(intact);
    }
    assert!(renamed_key_still_reads);
    assert!(unchanged);
    assert_eq!(rows, 1);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_corrupt_checkpoint_and_routing_metadata_are_rejected() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    attempt
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await
        .unwrap();
    let original = attempt.checkpoint().unwrap().to_json().unwrap();
    let mut corrupt = vec![
        b"not-json".to_vec(),
        b"[]".to_vec(),
        // Valid JSON and valid checkpoint claims, but not the canonical bytes registered by us.
        [b" \n".as_slice(), original.as_slice()].concat(),
    ];
    for (pointer, value) in [
        ("/reported_state", json!("unconfirmed")),
        ("/reported_confirmed_artifact_count", json!(1)),
        ("/version", json!(2)),
        ("/task/task_id", json!(Uuid::new_v4().to_string())),
    ] {
        let mut wire: Value = serde_json::from_slice(&original).unwrap();
        *wire.pointer_mut(pointer).unwrap() = value;
        corrupt.push(serde_json::to_vec(&wire).unwrap());
    }
    let mut outcomes = Vec::new();
    for bytes in corrupt {
        query("UPDATE collaboration_draft_intents SET checkpoint = $1")
            .bind(bytes)
            .execute(&f.journal_pool)
            .await
            .unwrap();
        let before = f.state().await;
        outcomes.push((
            f.get(&f.auth.learner_token, attempt.task_ref().task_id)
                .await
                .err(),
            before == f.state().await,
        ));
    }
    query("UPDATE collaboration_draft_intents SET checkpoint = $1")
        .bind(&original)
        .execute(&f.journal_pool)
        .await
        .unwrap();
    for column in ["source_namespace", "task_namespace", "artifact_namespace"] {
        let original: String = query(&format!(
            "SELECT {column} AS value FROM collaboration_draft_intents"
        ))
        .fetch_one(&f.journal_pool)
        .await
        .unwrap()
        .get("value");
        for value in [
            "different_configured_namespace".to_owned(),
            "n".repeat(4097),
        ] {
            query(&format!(
                "UPDATE collaboration_draft_intents SET {column} = $1"
            ))
            .bind(value)
            .execute(&f.journal_pool)
            .await
            .unwrap();
            let before = f.state().await;
            outcomes.push((
                f.get(&f.auth.learner_token, attempt.task_ref().task_id)
                    .await
                    .err(),
                before == f.state().await,
            ));
        }
        query(&format!(
            "UPDATE collaboration_draft_intents SET {column} = $1"
        ))
        .bind(original)
        .execute(&f.journal_pool)
        .await
        .unwrap();
    }
    // Corrupt only this disposable fixture after identifying its real checkpoint CHECK.
    // No production repair or assumption about the migration's auto-generated name is involved.
    let bound = query("SELECT c.conname, pg_get_constraintdef(c.oid) AS definition FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'checkpoint' WHERE c.conrelid = 'collaboration_draft_intents'::regclass AND c.contype = 'c' AND a.attnum = ANY(c.conkey)")
        .fetch_one(&f.journal_pool).await.unwrap();
    let bound_name: String = bound.get("conname");
    let bound_definition: String = bound.get("definition");
    f.sql(&format!(
        "ALTER TABLE collaboration_draft_intents DROP CONSTRAINT \"{}\"",
        bound_name.replace('"', "\"\"")
    ))
    .await;
    query("UPDATE collaboration_draft_intents SET checkpoint = $1")
        .bind(vec![b' '; 65537])
        .execute(&f.journal_pool)
        .await
        .unwrap();
    let before = f.state().await;
    outcomes.push((
        f.get(&f.auth.learner_token, attempt.task_ref().task_id)
            .await
            .err(),
        before == f.state().await,
    ));
    query("UPDATE collaboration_draft_intents SET checkpoint = $1")
        .bind(original)
        .execute(&f.journal_pool)
        .await
        .unwrap();
    f.sql(&format!(
        "ALTER TABLE collaboration_draft_intents ADD CONSTRAINT \"{}\" {bound_definition}",
        bound_name.replace('"', "\"\"")
    ))
    .await;
    let effects = counts(&f).await;
    f.cleanup().await;
    for (error, unchanged) in outcomes {
        assert_eq!(
            error,
            Some(SessionDraftIntentError::Journal(
                DraftIntentJournalError::InvalidRecord
            ))
        );
        assert!(unchanged);
    }
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_insert_faults_are_hit_and_rolled_back() {
    for mode in [
        "suppress",
        "checkpoint",
        "route",
        "account",
        "metadata",
        "redirect",
        "clone",
        "metadata_relation_clone",
    ] {
        let f = Fixture::ready().await;
        let attempt = prepare(&f, ITEMS);
        let alias = format!("intent_alias_{}", Uuid::new_v4().simple());
        let body = match mode {
            "suppress" => "RETURN NULL;".to_owned(),
            "checkpoint" => "NEW.checkpoint := convert_to('{}', 'UTF8');".to_owned(),
            "route" => "NEW.task_namespace := 'another_task_namespace';".to_owned(),
            "account" => format!("NEW.account_id := '{}';", Uuid::new_v4()),
            "metadata" => format!("UPDATE {}.collaboration_draft_intent_schema SET installation_id = '{}';", f.journal_schema, Uuid::new_v4()),
            "redirect" => "PERFORM set_config('search_path', 'public', true);".to_owned(),
            "metadata_relation_clone" => format!("ALTER TABLE {namespace}.collaboration_draft_intent_schema RENAME TO intent_metadata_original;
                CREATE TABLE {namespace}.collaboration_draft_intent_schema (LIKE {namespace}.intent_metadata_original INCLUDING ALL);
                INSERT INTO {namespace}.collaboration_draft_intent_schema SELECT * FROM {namespace}.intent_metadata_original;
                PERFORM nextval('{namespace}.intent_fault_calls');", namespace = f.journal_schema),
            _ => format!("ALTER SCHEMA {original} RENAME TO {alias}; CREATE SCHEMA {original};
                CREATE TABLE {original}.collaboration_draft_intent_schema (LIKE {alias}.collaboration_draft_intent_schema INCLUDING ALL);
                INSERT INTO {original}.collaboration_draft_intent_schema SELECT * FROM {alias}.collaboration_draft_intent_schema;
                CREATE TABLE {original}.collaboration_draft_intents (LIKE {alias}.collaboration_draft_intents INCLUDING ALL);", original = f.journal_schema),
        };
        f.fault(&body, false).await;
        if mode == "metadata_relation_clone" {
            // The metadata table, not the active INSERT target or its namespace, is replaced.
            f.sql("DROP TRIGGER intent_fault ON collaboration_draft_intents")
                .await;
            f.sql("CREATE TRIGGER intent_fault AFTER INSERT ON collaboration_draft_intents FOR EACH ROW EXECUTE FUNCTION intent_fault()").await;
        }
        let metadata_oid: i64 =
            query("SELECT 'collaboration_draft_intent_schema'::regclass::oid::bigint AS oid")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("oid");
        let metadata = f.metadata().await;
        let source = f.source_state().await;
        let before = f.state().await;
        let result = attempt
            .register_intent(&f.auth.learner_token, &f.intent_workspace)
            .await
            .err();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let hit = f.hit().await;
        // Sequence increments survive rollback: distinguish completed clone from DDL failure.
        let completed_clone = mode != "metadata_relation_clone"
            || query("SELECT last_value >= 2 AS completed FROM intent_fault_calls")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get::<bool, _>("completed");
        let same = before == f.state().await
            && metadata == f.metadata().await
            && source == f.source_state().await;
        let restored_oid: i64 =
            query("SELECT 'collaboration_draft_intent_schema'::regclass::oid::bigint AS oid")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("oid");
        let no_relation_alias: bool =
            query("SELECT to_regclass('intent_metadata_original') IS NULL AS absent")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("absent");
        let alias_count: i64 = query("SELECT count(*) AS n FROM pg_namespace WHERE nspname = $1")
            .bind(alias)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
        let effects = counts(&f).await;
        let ready = attempt.state() == &SessionDraftPublicationState::Ready;
        f.cleanup().await;
        assert!(
            result.is_some(),
            "fault {mode} must not confirm registration"
        );
        assert!(
            hit && completed_clone && same && ready,
            "fault {mode} must be reached, rolled back and leave the original attempt untouched"
        );
        assert_eq!(alias_count, 0);
        assert_eq!(restored_oid, metadata_oid);
        assert!(no_relation_alias);
        assert_eq!(effects, [0; 6]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_deferred_commit_failure_never_confirms_registration() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    f.fault("RAISE EXCEPTION 'synthetic intent commit failure';", true)
        .await;
    let metadata = f.metadata().await;
    let source = f.source_state().await;
    let result = attempt
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await
        .err();
    f.auth.base.drain().await;
    let hit = f.hit().await;
    let unchanged = metadata == f.metadata().await && source == f.source_state().await;
    let rows = f.count().await;
    let effects = counts(&f).await;
    let ready = attempt.state() == &SessionDraftPublicationState::Ready;
    f.cleanup().await;
    assert_eq!(
        result,
        Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::StorageUnavailable
        ))
    );
    assert!(hit && unchanged && ready);
    assert_eq!(rows, 0);
    assert_eq!(effects, [0; 6]);
}
