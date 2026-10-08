use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_requires_schema_two_and_exact_installation() {
    let legacy = intent_fixture::Fixture::ready().await;
    let original = prepare(&legacy, &[]);
    original
        .register_intent(&legacy.auth.learner_token, &legacy.intent_workspace)
        .await
        .unwrap();
    let mut legacy_errors = Vec::new();
    for task in [original.task_ref().task_id, id()] {
        legacy_errors.push(
            legacy
                .auth
                .source
                .inspect_session_draft_journal(
                    &legacy.auth.learner_token,
                    &legacy.intent_workspace,
                    task,
                )
                .await
                .err(),
        );
    }
    let legacy_rows = legacy.count().await;
    let legacy_version: i32 = query("SELECT version FROM collaboration_draft_intent_schema")
        .fetch_one(&legacy.journal_pool)
        .await
        .unwrap()
        .get("version");
    legacy.cleanup().await;
    for error in legacy_errors {
        assert_eq!(
            error,
            Some(SessionDraftIntentError::Journal(
                DraftIntentJournalError::UnsupportedSchema
            ))
        );
    }
    assert_eq!((legacy_rows, legacy_version), (1, 1));

    let f = Fixture::ready().await;
    let attempt = f.journaled(&[]).await;
    let other = DraftIntentJournal::new(f.journal_pool.clone(), scope(), Uuid::new_v4()).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, other, f.workspace.clone()).unwrap();
    let before = snapshot(&f).await;
    let mismatch = f
        .auth
        .source
        .inspect_session_draft_journal(
            &f.auth.learner_token,
            &workspace,
            attempt.task_ref().task_id,
        )
        .await
        .err();
    let intact = before == snapshot(&f).await;
    let empty_name = format!("uninstalled_journal_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {empty_name}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let pool = input_fixture::pool_for(&empty_name).await;
    let journal = DraftIntentJournal::new(pool.clone(), scope(), Uuid::new_v4()).unwrap();
    let empty =
        SessionDraftIntentWorkspace::new(&empty_name, journal, f.workspace.clone()).unwrap();
    let absent_schema = f
        .auth
        .source
        .inspect_session_draft_journal(&f.auth.learner_token, &empty, id())
        .await
        .err();
    let tables: i64 =
        query("SELECT count(*) AS n FROM pg_class WHERE relnamespace = $1::regnamespace")
            .bind(&empty_name)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
    drop(empty);
    pool.close().await;
    query(&format!("DROP SCHEMA {empty_name} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
    assert_eq!(
        mismatch,
        Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::IncarnationMismatch
        ))
    );
    assert!(intact && absent_schema.is_some());
    assert_eq!(tables, 0);
    for mode in ["primary_key", "nullable", "workspace", "authority"] {
        let f = Fixture::ready().await;
        let attempt = f.journaled(&[]).await;
        let expected = match mode {
            "primary_key" => {
                let name: String = query("SELECT conname FROM pg_constraint WHERE conrelid = 'collaboration_draft_dispatch_steps'::regclass AND contype = 'p'")
                    .fetch_one(&f.journal_pool).await.unwrap().get("conname");
                f.sql(&format!(
                    "ALTER TABLE collaboration_draft_dispatch_steps DROP CONSTRAINT \"{}\"",
                    name.replace('"', "\"\"")
                ))
                .await;
                DraftIntentJournalError::UnsupportedSchema
            }
            "nullable" => {
                f.sql("ALTER TABLE collaboration_draft_dispatch_steps ALTER COLUMN committed DROP NOT NULL").await;
                DraftIntentJournalError::UnsupportedSchema
            }
            _ => {
                let column = if mode == "workspace" {
                    "workspace_id"
                } else {
                    "authority_id"
                };
                f.sql(&format!(
                    "UPDATE collaboration_draft_intent_schema SET {column} = '{}'",
                    Uuid::new_v4()
                ))
                .await;
                DraftIntentJournalError::ScopeMismatch
            }
        };
        let before = snapshot(&f).await;
        let error = inspect(&f, &f.auth.learner_token, attempt.task_ref().task_id)
            .await
            .err();
        let unchanged = before == snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(
            error,
            Some(SessionDraftIntentError::Journal(expected)),
            "{mode}"
        );
        assert!(unchanged);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_rejects_changed_namespace_routes() {
    let f = Fixture::ready().await;
    let attempt = f.journaled(&[]).await;
    let mut outcomes = Vec::new();
    for column in ["source_namespace", "task_namespace", "artifact_namespace"] {
        let original: String = query(&format!(
            "SELECT {column} AS v FROM collaboration_draft_intents"
        ))
        .fetch_one(&f.journal_pool)
        .await
        .unwrap()
        .get("v");
        for changed in ["other_namespace".to_owned(), "n".repeat(4097)] {
            query(&format!(
                "UPDATE collaboration_draft_intents SET {column} = $1"
            ))
            .bind(changed)
            .execute(&f.journal_pool)
            .await
            .unwrap();
            let before = snapshot(&f).await;
            outcomes.push((
                inspect(&f, &f.auth.learner_token, attempt.task_ref().task_id)
                    .await
                    .err(),
                before == snapshot(&f).await,
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
    f.cleanup().await;
    for (error, intact) in outcomes {
        assert_eq!(
            error,
            Some(SessionDraftIntentError::Journal(
                DraftIntentJournalError::InvalidRecord
            ))
        );
        assert!(intact);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_rejects_noncanonical_and_oversized_intents() {
    let f = Fixture::ready().await;
    let attempt = f.journaled(ITEMS).await;
    let original = attempt.checkpoint().unwrap().to_json().unwrap();
    let mut corrupt = vec![
        b"[]".to_vec(),
        b"not-json".to_vec(),
        [b" \n".as_slice(), &original].concat(),
    ];
    for (pointer, value) in [
        ("/reported_state", serde_json::json!("complete")),
        ("/reported_confirmed_artifact_count", serde_json::json!(1)),
        (
            "/task/task_id",
            serde_json::json!(Uuid::new_v4().to_string()),
        ),
    ] {
        let mut json: serde_json::Value = serde_json::from_slice(&original).unwrap();
        *json.pointer_mut(pointer).unwrap() = value;
        corrupt.push(serde_json::to_vec(&json).unwrap());
    }
    let bound: String = query("SELECT c.conname FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'checkpoint' WHERE c.conrelid = 'collaboration_draft_intents'::regclass AND c.contype = 'c' AND a.attnum = ANY(c.conkey)")
        .fetch_one(&f.journal_pool).await.unwrap().get("conname");
    f.sql(&format!(
        "ALTER TABLE collaboration_draft_intents DROP CONSTRAINT \"{}\"",
        bound.replace('"', "\"\"")
    ))
    .await;
    corrupt.push(vec![b' '; 65537]);
    let mut outcomes = Vec::new();
    for bytes in corrupt {
        query("UPDATE collaboration_draft_intents SET checkpoint = $1")
            .bind(bytes)
            .execute(&f.journal_pool)
            .await
            .unwrap();
        let before = snapshot(&f).await;
        outcomes.push((
            inspect(&f, &f.auth.learner_token, attempt.task_ref().task_id)
                .await
                .err(),
            before == snapshot(&f).await,
        ));
    }
    f.cleanup().await;
    for (error, intact) in outcomes {
        assert_eq!(
            error,
            Some(SessionDraftIntentError::Journal(
                DraftIntentJournalError::InvalidRecord
            ))
        );
        assert!(intact);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_rejects_invalid_step_sequences_and_bounds() {
    let f = Fixture::ready().await;
    let items = vec![("bounded.txt", "text", "bounded"); 32];
    let attempt = f.journaled(&items).await;
    let task = attempt.task_ref();
    let bound: String = query("SELECT c.conname FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'ordinal' WHERE c.conrelid = 'collaboration_draft_dispatch_steps'::regclass AND c.contype = 'c' AND a.attnum = ANY(c.conkey)")
        .fetch_one(&f.journal_pool).await.unwrap().get("conname");
    f.sql(&format!(
        "ALTER TABLE collaboration_draft_dispatch_steps DROP CONSTRAINT \"{}\"",
        bound.replace('"', "\"\"")
    ))
    .await;
    let mut outcomes = Vec::new();
    for mode in [
        "gap",
        "negative",
        "nil_nonce",
        "v1_nonce",
        "variant_nonce",
        "unknown_then_true",
        "two_unknown",
        "overflow_34",
        "complete_33",
        "unknown_task_33",
    ] {
        f.sql("DELETE FROM collaboration_draft_dispatch_steps")
            .await;
        let mut rows = match mode {
            "gap" => vec![(1, true)],
            "negative" => vec![(-1, false)],
            "unknown_then_true" => vec![(0, false), (1, true)],
            "two_unknown" => vec![(0, false), (1, false)],
            "overflow_34" => (0..34).map(|n| (n, true)).collect(),
            "complete_33" | "unknown_task_33" => (0..33)
                .map(|n| (n, mode == "complete_33" || n < 32))
                .collect(),
            _ => vec![(0, false)],
        };
        for (ordinal, committed) in rows.drain(..) {
            let nonce = match mode {
                "nil_nonce" => Uuid::nil(),
                "v1_nonce" => Uuid::parse_str("1223c6e9-823d-1cea-998a-a92f019b35fc").unwrap(),
                "variant_nonce" => {
                    let mut bytes = *Uuid::new_v4().as_bytes();
                    bytes[8] = 0;
                    Uuid::from_bytes(bytes)
                }
                _ => Uuid::new_v4(),
            };
            query("INSERT INTO collaboration_draft_dispatch_steps (task_id, ordinal, nonce, committed) VALUES ($1, $2, $3, $4)")
                .bind(task.task_id.as_uuid()).bind(ordinal).bind(nonce).bind(committed).execute(&f.journal_pool).await.unwrap();
        }
        let before = snapshot(&f).await;
        outcomes.push((
            mode,
            inspect(&f, &f.auth.learner_token, task.task_id).await,
            before == snapshot(&f).await,
        ));
    }
    f.cleanup().await;
    for (mode, result, intact) in outcomes {
        assert!(intact, "{mode}");
        if mode == "complete_33" || mode == "unknown_task_33" {
            let value = result.unwrap().unwrap();
            assert_eq!(value.4, if mode == "complete_33" { 33 } else { 32 });
            assert_eq!(value.6, mode == "complete_33");
            assert_eq!(
                value.5,
                if mode == "complete_33" {
                    None
                } else {
                    Some(SessionDraftPublicationStep::Task { reference: task })
                }
            );
        } else {
            assert_eq!(
                result,
                Err(SessionDraftIntentError::Journal(
                    DraftIntentJournalError::InvalidRecord
                )),
                "{mode}"
            );
        }
    }
    // Synthetic rows exercise parser bounds, not evidence of 33 actual resource writes.
    let f = Fixture::ready().await;
    let empty = f.journaled(&[]).await;
    let two = f.journaled(ITEMS).await;
    for ordinal in 0..2 {
        query("INSERT INTO collaboration_draft_dispatch_steps (task_id, ordinal, nonce, committed) VALUES ($1, $2, $3, TRUE)")
            .bind(empty.task_ref().task_id.as_uuid()).bind(ordinal).bind(Uuid::new_v4()).execute(&f.journal_pool).await.unwrap();
    }
    let before = snapshot(&f).await;
    let short_plan = inspect(&f, &f.auth.learner_token, empty.task_ref().task_id).await;
    let short_intact = before == snapshot(&f).await;
    f.sql("DELETE FROM collaboration_draft_dispatch_steps")
        .await;
    let unique: String = query("SELECT c.conname FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'nonce' WHERE c.conrelid = 'collaboration_draft_dispatch_steps'::regclass AND c.contype = 'u' AND a.attnum = ANY(c.conkey)")
        .fetch_one(&f.journal_pool).await.unwrap().get("conname");
    f.sql(&format!(
        "ALTER TABLE collaboration_draft_dispatch_steps DROP CONSTRAINT \"{}\"",
        unique.replace('"', "\"\"")
    ))
    .await;
    let nonce = Uuid::new_v4();
    for ordinal in 0..2 {
        query("INSERT INTO collaboration_draft_dispatch_steps (task_id, ordinal, nonce, committed) VALUES ($1, $2, $3, TRUE)")
            .bind(two.task_ref().task_id.as_uuid()).bind(ordinal).bind(nonce).execute(&f.journal_pool).await.unwrap();
    }
    let before = snapshot(&f).await;
    let duplicate_nonce = inspect(&f, &f.auth.learner_token, two.task_ref().task_id).await;
    let duplicate_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_eq!(
        short_plan,
        Err(SessionDraftIntentError::Journal(
            DraftIntentJournalError::InvalidRecord
        ))
    );
    // Removing UNIQUE is rejected by the earlier schema guard; this does not claim the row-level
    // duplicate-nonce branch was reached, and never tampers with PostgreSQL system catalogs.
    assert_eq!(
        duplicate_nonce,
        Err(SessionDraftIntentError::Journal(
            DraftIntentJournalError::UnsupportedSchema
        ))
    );
    assert!(short_intact && duplicate_intact);
}
