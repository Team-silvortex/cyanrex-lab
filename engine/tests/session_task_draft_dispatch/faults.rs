use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_phase_a_faults_never_reach_resource_writes() {
    for mode in ["suppress", "nonce", "ordinal", "committed", "commit"] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(ITEMS).await;
        let body = match mode {
            "suppress" => "RETURN NULL;".to_owned(),
            "nonce" => format!("NEW.nonce := '{}';", Uuid::new_v4()),
            "ordinal" => "NEW.ordinal := 1;".to_owned(),
            "committed" => "NEW.committed := TRUE;".to_owned(),
            _ => "RAISE EXCEPTION 'synthetic phase A commit failure';".to_owned(),
        };
        f.step_fault("INSERT", &body, mode == "commit").await;
        let intent_before = f.state().await;
        let result = attempt.advance(&f.auth.learner_token).await;
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let hit = f.step_hit().await;
        let steps = f.steps().await;
        let effects = counts(&f).await;
        let unchanged = intent_before == f.state().await;
        let terminal = unknown(&attempt);
        let retry = attempt.advance(&f.auth.learner_token).await;
        f.cleanup().await;
        assert!(result.is_err() && hit && unchanged && terminal, "{mode}");
        assert_eq!(retry, Err(stopped()));
        assert!(steps.is_empty());
        assert_eq!(effects, [0; 6]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_artifact_failure_preserves_unknown_and_confirmed_prefix() {
    for deferred in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(ITEMS).await;
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let prefix = attempt.confirmed_artifacts().to_vec();
        f.resource_fault(
            false,
            if deferred {
                "RAISE EXCEPTION 'synthetic artifact commit failure';"
            } else {
                "RETURN NULL;"
            },
            deferred,
        )
        .await;
        let result = attempt.advance(&f.auth.learner_token).await;
        f.auth.base.drain().await;
        let hit = f.resource_hit(false).await;
        let steps = f.steps().await;
        let effects = counts(&f).await;
        let retained = attempt.confirmed_artifacts() == prefix;
        let terminal = unknown(&attempt);
        let retry = attempt.advance(&f.auth.learner_token).await;
        let stable = steps == f.steps().await && effects == counts(&f).await;
        f.cleanup().await;
        assert!(result.is_err() && hit && retained && terminal && stable);
        assert_eq!(retry, Err(stopped()));
        assert_eq!(
            steps.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(),
            [(0, true), (1, false)]
        );
        assert_eq!(&effects[..5], &[1, 1, 1, 0, 0]);
        // Artifact files are not transactional: the second already-published file remains private.
        assert_eq!(effects[5], 2);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_task_failure_preserves_all_confirmed_artifacts() {
    for deferred in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(ITEMS).await;
        for _ in ITEMS {
            attempt.advance(&f.auth.learner_token).await.unwrap();
        }
        let prefix = attempt.confirmed_artifacts().to_vec();
        f.resource_fault(
            true,
            if deferred {
                "RAISE EXCEPTION 'synthetic task commit failure';"
            } else {
                "RETURN NULL;"
            },
            deferred,
        )
        .await;
        let result = attempt.advance(&f.auth.learner_token).await;
        f.auth.base.drain().await;
        let hit = f.resource_hit(true).await;
        let steps = f.steps().await;
        let effects = counts(&f).await;
        let retained =
            attempt.confirmed_artifacts() == prefix && attempt.confirmed_task().is_none();
        let terminal = unknown(&attempt);
        let retry = attempt.advance(&f.auth.learner_token).await;
        f.cleanup().await;
        assert!(result.is_err() && hit && retained && terminal);
        assert_eq!(retry, Err(stopped()));
        assert_eq!(
            steps.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(),
            [(0, true), (1, true), (2, false)]
        );
        assert_eq!(effects, [2, 2, 2, 0, 0, 2]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_phase_b_marker_and_intent_changes_roll_back_resources() {
    for task in [false, true] {
        for mode in [
            "nonce",
            "suppress",
            "checkpoint",
            "route",
            "installation",
            "after_write_checkpoint",
            "after_write_marker",
        ] {
            let f = Fixture::ready().await;
            let mut attempt = f.journaled(if task { &[] } else { ITEMS }).await;
            let body = match mode {
                "nonce" => format!("NEW.nonce := '{}';", Uuid::new_v4()),
                "suppress" => "RETURN NULL;".to_owned(),
                "checkpoint" | "after_write_checkpoint" => format!("UPDATE {}.collaboration_draft_intents SET checkpoint = convert_to('{{}}', 'UTF8');", f.journal_schema),
                "route" => format!("UPDATE {}.collaboration_draft_intents SET task_namespace = 'changed_tasks';", f.journal_schema),
                "after_write_marker" => format!("UPDATE {}.collaboration_draft_dispatch_steps SET committed = FALSE;", f.journal_schema),
                _ => format!("UPDATE {}.collaboration_draft_intent_schema SET installation_id = '{}';", f.journal_schema, Uuid::new_v4()),
            };
            let after_write = mode.starts_with("after_write_");
            if after_write {
                // Marker CAS and resource rows have been written before this outbox fault.
                f.resource_fault(task, &body, false).await;
            } else {
                f.step_fault("UPDATE", &body, false).await;
            }
            let before = f.state().await;
            let metadata = f.metadata().await;
            let result = attempt.advance(&f.auth.learner_token).await;
            f.auth.base.drain().await;
            let hit = if after_write {
                f.resource_hit(task).await
            } else {
                f.step_hit().await
            };
            let steps = f.steps().await;
            let effects = counts(&f).await;
            let intact = before == f.state().await && metadata == f.metadata().await;
            let terminal = unknown(&attempt);
            f.cleanup().await;
            assert!(
                result.is_err() && hit && intact && terminal,
                "task={task}, {mode}"
            );
            assert_eq!(steps.len(), 1);
            assert!(!steps[0].2);
            assert_eq!(&effects[..5], &[0; 5]);
            // Depending on the rejection point an immutable blob may already exist; no cleanup claim.
            assert!(effects[5] <= i64::from(!task));
            if after_write && !task {
                assert_eq!(effects[5], 1);
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_damaged_marker_shape_and_prefix_are_not_retried() {
    for mode in [
        "prefix_unknown",
        "prefix_missing",
        "prefix_ordinal",
        "nullable",
        "primary_key",
        "steps_relation_clone",
        "overflow_34",
        "after_write_prefix_nonce",
    ] {
        let f = Fixture::ready().await;
        // With 32 payloads the legitimate global bound is exactly 33 steps. A LIMIT 33
        // implementation would see only a valid-shaped prefix and misclassify the extra row.
        let items = if mode == "overflow_34" {
            vec![("bounded.txt", "text", "bounded bytes"); 32]
        } else {
            ITEMS.to_vec()
        };
        let mut attempt = f.journaled(&items).await;
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let confirmed_prefix = attempt.confirmed_artifacts().to_vec();
        let original_oid: i64 =
            query("SELECT 'collaboration_draft_dispatch_steps'::regclass::oid::bigint AS oid")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("oid");
        match mode {
            "prefix_unknown" => f.sql("UPDATE collaboration_draft_dispatch_steps SET committed = FALSE").await,
            "prefix_missing" => f.sql("DELETE FROM collaboration_draft_dispatch_steps").await,
            "prefix_ordinal" => f.sql("UPDATE collaboration_draft_dispatch_steps SET ordinal = 2").await,
            "nullable" => f.sql("ALTER TABLE collaboration_draft_dispatch_steps ALTER COLUMN committed DROP NOT NULL").await,
            "primary_key" => {
                let name: String = query("SELECT conname FROM pg_constraint WHERE conrelid = 'collaboration_draft_dispatch_steps'::regclass AND contype = 'p'").fetch_one(&f.journal_pool).await.unwrap().get("conname");
                f.sql(&format!("ALTER TABLE collaboration_draft_dispatch_steps DROP CONSTRAINT \"{}\"", name.replace('"', "\"\""))).await;
            }
            "overflow_34" => {
                let name: String = query("SELECT c.conname FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'ordinal' WHERE c.conrelid = 'collaboration_draft_dispatch_steps'::regclass AND c.contype = 'c' AND a.attnum = ANY(c.conkey)")
                    .fetch_one(&f.journal_pool).await.unwrap().get("conname");
                f.sql(&format!("ALTER TABLE collaboration_draft_dispatch_steps DROP CONSTRAINT \"{}\"", name.replace('"', "\"\""))).await;
                for ordinal in 1..=33 {
                    query("INSERT INTO collaboration_draft_dispatch_steps (task_id, ordinal, nonce, committed) VALUES ($1, $2, $3, TRUE)")
                        .bind(attempt.task_ref().task_id.as_uuid()).bind(ordinal).bind(Uuid::new_v4())
                        .execute(&f.journal_pool).await.unwrap();
                }
            }
            "after_write_prefix_nonce" => {
                // B's current ordinal 1 and nonce remain untouched. Only the previously
                // committed ordinal 0 is corrupted after the second resource has been written.
                f.resource_fault(false, &format!("UPDATE {}.collaboration_draft_dispatch_steps SET nonce = '{}' WHERE ordinal = 0;",
                    f.journal_schema, Uuid::new_v4()), false).await;
            }
            _ => {
                // Replace a no-longer-active relation from the resource outbox trigger, after
                // B's marker UPDATE. LIKE omits FKs, so restore the real FK explicitly: a fresh
                // shape check alone must not be why this otherwise equivalent clone is rejected.
                f.resource_fault(false, &format!("ALTER TABLE {ns}.collaboration_draft_dispatch_steps RENAME TO dispatch_steps_original;
                    CREATE TABLE {ns}.collaboration_draft_dispatch_steps (LIKE {ns}.dispatch_steps_original INCLUDING ALL);
                    ALTER TABLE {ns}.collaboration_draft_dispatch_steps ADD FOREIGN KEY (task_id) REFERENCES {ns}.collaboration_draft_intents(task_id);
                    INSERT INTO {ns}.collaboration_draft_dispatch_steps SELECT * FROM {ns}.dispatch_steps_original;", ns = f.journal_schema), false).await;
            }
        }
        let before = f.steps().await;
        let effects = counts(&f).await;
        let result = attempt.advance(&f.auth.learner_token).await;
        let terminal = unknown(&attempt);
        let retry = attempt.advance(&f.auth.learner_token).await;
        let after = f.steps().await;
        let after_effects = counts(&f).await;
        let post_write_fault = matches!(mode, "steps_relation_clone" | "after_write_prefix_nonce");
        let fault_hit = !post_write_fault || f.resource_hit(false).await;
        let restored_oid: i64 =
            query("SELECT 'collaboration_draft_dispatch_steps'::regclass::oid::bigint AS oid")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("oid");
        let no_clone_alias: bool =
            query("SELECT to_regclass('dispatch_steps_original') IS NULL AS absent")
                .fetch_one(&f.journal_pool)
                .await
                .unwrap()
                .get("absent");
        let intact = if post_write_fault {
            before.len() == 1
                && after.len() == 2
                && before[0] == after[0]
                && !after[1].2
                && after[1].0 == 1
                && effects[..5] == after_effects[..5]
                && after_effects[5] == 2
        } else {
            before == after && effects == after_effects
        };
        let prefix = attempt.confirmed_artifacts().len();
        let confirmed_intact = attempt.confirmed_artifacts() == confirmed_prefix;
        f.cleanup().await;
        assert!(
            result.is_err() && terminal && intact && fault_hit && confirmed_intact,
            "{mode}"
        );
        assert_eq!(restored_oid, original_oid);
        assert!(no_clone_alias);
        if mode == "overflow_34" {
            assert_eq!(before.len(), 34);
            // Conflict from the truncated 33-row prefix is not the overflow classification.
            assert_eq!(
                result,
                Err(SessionDraftDispatchError::Intent(
                    SessionDraftIntentError::Journal(DraftIntentJournalError::InvalidRecord)
                ))
            );
        }
        assert_eq!(retry, Err(stopped()));
        assert_eq!(prefix, 1);
    }
}
