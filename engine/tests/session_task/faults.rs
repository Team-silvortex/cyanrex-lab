use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_scope_and_missing_audits_cannot_authorize_writes() {
    let f = Fixture::ready().await;
    let foreign = SessionTaskWorkspace::new(
        &f.schema,
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
    )
    .unwrap();
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &foreign, id(), "Wrong workspace")
        .await
        .is_err());
    let source = DurableAuthSource::new(f.auth.base.pool.clone(), id());
    assert!(source
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Wrong source")
        .await
        .is_err());
    f.sql("UPDATE collaboration_task_schema SET workspace_id = '267c6b0f-182c-4980-a925-cdc967327c57'").await;
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Wrong target")
            .await,
        Err(SessionTaskError::Task(TaskStoreError::ScopeMismatch))
    );
    query("UPDATE collaboration_task_schema SET workspace_id = $1")
        .bind(scope().workspace_id.as_uuid())
        .execute(&f.pool)
        .await
        .unwrap();
    f.auth
        .sql("UPDATE collaboration_identity_schema SET version = 1")
        .await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            "Missing identity audit"
        )
        .await
        .is_err());
    f.auth
        .sql("UPDATE collaboration_identity_schema SET version = 2")
        .await;
    f.auth
        .sql("UPDATE collaboration_access_schema SET version = 1")
        .await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            "Missing policy audit"
        )
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_suspended_members_and_archived_workspaces_are_denied() {
    let f = Fixture::ready().await;
    let current = f.auth.current(f.auth.learner.binding.principal_id).await;
    let mut desired = current.policy;
    desired.membership.status = MembershipStatus::Suspended;
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: Some(current.revision),
                desired,
            },
        )
        .await
        .unwrap();
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Suspended")
        .await
        .is_err());
    f.auth
        .sql("UPDATE collaboration_workspaces SET status = 'archived'")
        .await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(
            &f.auth.manager_token,
            &f.workspace,
            id(),
            "Archived manager"
        )
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_event_and_deferred_commit_failures_publish_nothing() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION suppress_task_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
    f.sql("CREATE TRIGGER suppress_task_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION suppress_task_event()").await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Suppressed")
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.sql("DROP TRIGGER suppress_task_event ON collaboration_task_outbox")
        .await;
    let first = f.create(&f.auth.learner_token).await;
    f.sql("CREATE FUNCTION fail_task_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_task_commit AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_task_commit()").await;
    assert!(f
        .auth
        .source
        .transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            first.reference.task_id,
            first.revision,
            TaskStatus::Ready
        )
        .await
        .is_err());
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(&f.auth.learner_token, &f.workspace, first.reference.task_id)
            .await
            .unwrap(),
        Some(first)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_post_write_session_or_membership_changes_roll_back() {
    let f = Fixture::ready().await;
    let actor = f.auth.learner.principal.reference;
    for body in [
        format!("DELETE FROM {}.sessions", f.auth.base.schema),
        format!("UPDATE {}.collaboration_memberships SET status = 'suspended' WHERE principal_id = '{}'", f.auth.base.schema, actor.principal_id),
        format!("UPDATE {}.collaboration_workspaces SET status = 'archived'", f.auth.base.schema),
        format!("UPDATE {}.collaboration_auth_source_schema SET authority_id = '{}'", f.auth.base.schema, Uuid::new_v4()),
    ] {
        f.sql(&format!("CREATE OR REPLACE FUNCTION change_task_authority() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {body}; RETURN NEW; END $$")).await;
        f.sql("CREATE TRIGGER change_task_authority BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION change_task_authority()").await;
        assert!(f.auth.source.create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Must roll back").await.is_err());
        assert_eq!(f.count("collaboration_tasks").await, 0);
        assert_eq!(f.count("collaboration_task_outbox").await, 0);
        f.sql("DROP TRIGGER change_task_authority ON collaboration_task_outbox").await;
        assert!(f.auth.source.validate_session(&f.auth.learner_token).await.unwrap().is_some());
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_source_rls_and_temporary_shadows_fail_closed() {
    let f = Fixture::ready().await;
    f.auth
        .sql("ALTER TABLE sessions ENABLE ROW LEVEL SECURITY")
        .await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "RLS")
        .await
        .is_err());
    f.auth
        .sql("ALTER TABLE sessions DISABLE ROW LEVEL SECURITY")
        .await;
    let schema = f.auth.base.schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false)")
                    .bind(schema)
                    .execute(&mut *connection)
                    .await?;
                query("CREATE TEMP TABLE sessions (LIKE sessions INCLUDING ALL)")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    let source = DurableAuthSource::new(pool.clone(), authority());
    assert_eq!(
        source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Shadow")
            .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    pool.close().await;
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_search_path_changes_are_rejected_and_pool_scope_is_restored() {
    let f = Fixture::ready().await;
    let clone_schema = format!("{}_clone", f.schema);
    query(&format!("CREATE SCHEMA {clone_schema}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone_schema).await;
    TaskStore::new(clone_pool.clone(), scope())
        .install_empty_namespace()
        .await
        .unwrap();
    f.sql(&format!(
        "CREATE FUNCTION change_task_path() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        INSERT INTO {clone_schema}.collaboration_tasks
            SELECT * FROM {}.collaboration_tasks WHERE task_id = NEW.task_id;
        INSERT INTO {clone_schema}.collaboration_task_outbox
            (event_id, task_id, revision, actor_id, event_type, snapshot)
            VALUES (NEW.event_id, NEW.task_id, NEW.revision, NEW.actor_id, NEW.event_type, NEW.snapshot);
        PERFORM set_config('search_path', '{clone_schema}', true); RETURN NEW; END $$",
        f.schema,
    )).await;
    f.sql("CREATE TRIGGER change_task_path BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION change_task_path()").await;
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Wrong path")
            .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    f.auth.base.drain().await;
    let namespace: String = query("SELECT current_schema()::text AS name")
        .fetch_one(&f.auth.base.pool)
        .await
        .unwrap()
        .get("name");
    assert_eq!(namespace, f.auth.base.schema);
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let count: i64 = query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&clone_pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(count, 0);
    }
    clone_pool.close().await;
    query(&format!("DROP SCHEMA {clone_schema} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}
