use super::fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_missing_foreign_and_missing_task_scope_remain_not_visible() {
    for task in [false, true] {
        for foreign in [false, true] {
            let f = Fixture::ready().await;
            let mut attempt = if task {
                unknown_task(&f, 1, &[]).await
            } else {
                unknown_artifact(&f, 1, ITEMS, 0).await
            };
            if foreign {
                if task {
                    create_target(&f, &attempt, &f.auth.manager_token, 1, &[]).await;
                } else {
                    publish_target(&f, &attempt, &f.auth.manager_token, ITEMS, 0).await;
                }
            }
            assert_eq!(
                observe_preserved(&f, &attempt).await,
                report(&attempt, SessionDraftObservedOutcome::NotVisible)
            );
            stopped(&f, &mut attempt).await;
            drop(attempt);
            f.cleanup().await;
        }
    }
    let f = Fixture::ready().await;
    let mut attempt = unknown_task(&f, 0, &[]).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    assert_eq!(
        observe_preserved(&f, &attempt).await,
        report(&attempt, SessionDraftObservedOutcome::NotVisible),
        "a missing Task read does not inspect the Artifact namespace"
    );
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
        .await;
    create_target(&f, &attempt, &f.auth.learner_token, 0, &[]).await;
    assert_eq!(
        observe_preserved(&f, &attempt).await,
        report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate)
    );
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    assert_eq!(
        observe_preserved(&f, &attempt).await,
        Err(SessionDraftPublicationError::Task(
            SessionTaskError::Artifact(ArtifactStoreError::UnsupportedSchema)
        )),
        "an existing empty Task still validates its Artifact namespace"
    );
    stopped(&f, &mut attempt).await;
    drop(attempt);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_effective_commit_failures_and_invalid_definition_bytes_remain_typed(
) {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = prepare_catalog(&f, 0, if task { &[] } else { ITEMS });
        let (schema, pool, table) = if task {
            (&f.task_schema, &f.task_pool, "collaboration_task_outbox")
        } else {
            (
                &f.artifact_schema,
                &f.artifact_pool,
                "collaboration_artifact_outbox",
            )
        };
        query("CREATE SEQUENCE observe_fault_calls")
            .execute(pool)
            .await
            .unwrap();
        query(&format!("CREATE FUNCTION observe_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{schema}.observe_fault_calls'); RAISE EXCEPTION 'private deferred failure'; END $$")).execute(pool).await.unwrap();
        query(&format!("CREATE CONSTRAINT TRIGGER observe_fault AFTER INSERT ON {table} DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION observe_fault()")).execute(pool).await.unwrap();
        let failed = attempt.advance(&f.auth.learner_token).await;
        f.auth.base.drain().await;
        let row = query("SELECT is_called, last_value FROM observe_fault_calls")
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(row.get::<bool, _>("is_called"));
        assert_eq!(row.get::<i64, _>("last_value"), 1);
        let error = SessionCommandError::Authentication(DurableAuthError::StorageUnavailable);
        assert_eq!(
            failed,
            Err(if task {
                SessionDraftPublicationError::Task(SessionTaskError::Session(error))
            } else {
                SessionDraftPublicationError::Artifact(SessionArtifactError::Session(error))
            })
        );
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            report(&attempt, SessionDraftObservedOutcome::NotVisible)
        );
        assert_eq!(counts(&f).await, if task { [0,0,0,0,0,0] } else { [0,0,0,0,0,1] }, "this injected transaction's rollback is separately measured, never inferred from NotVisible");
        if !task {
            assert_eq!(
                fs::read(f.blob(&attempt.allocated_artifacts()[0])).unwrap(),
                ITEMS[0].2.as_bytes()
            );
        }
        stopped(&f, &mut attempt).await;
        drop(attempt);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
    for task in [false, true] {
        for missing in [false, true] {
            let f = Fixture::ready().await;
            let mut attempt = if task {
                let attempt = unknown_task(&f, 1, ITEMS).await;
                create_target(&f, &attempt, &f.auth.learner_token, 1, ITEMS).await;
                attempt
            } else {
                let attempt = unknown_artifact(&f, 1, ITEMS, 0).await;
                publish_target(&f, &attempt, &f.auth.learner_token, ITEMS, 0).await;
                attempt
            };
            damage(&f, &attempt.allocated_artifacts()[0], missing);
            let error = if task {
                SessionDraftPublicationError::Task(SessionTaskError::Artifact(
                    ArtifactStoreError::BlobUnavailable,
                ))
            } else {
                SessionDraftPublicationError::Artifact(SessionArtifactError::Artifact(
                    ArtifactStoreError::BlobUnavailable,
                ))
            };
            assert_eq!(observe_preserved(&f, &attempt).await, Err(error));
            stopped(&f, &mut attempt).await;
            drop(attempt);
            f.cleanup().await;
        }
    }
    for (entry, path) in [
        (0, "{definition,summary}"),
        (0, "{definition,evidence_schema,version}"),
        (0, "{definition,assessment_policy,version}"),
        (0, "{definition}"),
        (0, "binding-pair"),
        (1, "binding-pair"),
    ] {
        let f = Fixture::ready().await;
        let mut attempt = unknown_task(&f, entry, &[]).await;
        let created = create_target(&f, &attempt, &f.auth.learner_token, entry, &[]).await;
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate),
            "valid full definition must match before its effective mutation"
        );
        let (path, replacement) = match path {
            "binding-pair" => {
                let mut changed = created.task.definition.clone().unwrap();
                if entry == 0 {
                    changed.evidence_schema = None;
                    changed.assessment_policy = None;
                } else {
                    changed.evidence_schema = values()[0].evidence_schema.clone();
                    changed.assessment_policy = values()[0].assessment_policy.clone();
                }
                assert_ne!(changed, created.task.definition.clone().unwrap());
                ("{definition}", serde_json::to_string(&changed).unwrap())
            }
            "{definition,summary}" => (path, "\"Changed full metadata\"".into()),
            "{definition}" => (path, "null".into()),
            _ => (path, "9".into()),
        };
        for table in ["collaboration_tasks", "collaboration_task_outbox"] {
            assert_eq!(query(&format!("UPDATE {table} SET snapshot = jsonb_set(snapshot::jsonb, $2::text[], $3::jsonb)::text WHERE task_id = $1"))
                .bind(created.task.reference.task_id.as_uuid()).bind(path).bind(&replacement).execute(&f.task_pool).await.unwrap().rows_affected(), 1);
        }
        let changed: String = query("SELECT snapshot FROM collaboration_tasks WHERE task_id = $1")
            .bind(created.task.reference.task_id.as_uuid())
            .fetch_one(&f.task_pool)
            .await
            .unwrap()
            .get("snapshot");
        assert_ne!(
            serde_json::from_str::<TaskSnapshot>(&changed).unwrap(),
            created.task,
            "metadata injection must change actual stored full definition"
        );
        assert_eq!(
            observe_preserved(&f, &attempt).await,
            Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
                TaskStoreError::InvalidRecord
            ))),
            "invalid metadata/manual None stays an error, not a valid Differs observation"
        );
        stopped(&f, &mut attempt).await;
        drop(attempt);
        f.cleanup().await;
    }
}
