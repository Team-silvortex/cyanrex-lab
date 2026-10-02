use super::*;

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_schema_scope_and_row_security_fail_closed_without_installing() {
    let _guard = guard().await;
    let db = Fixture::new().await;
    assert_eq!(
        db.source.reconcile_authority(scope()).await.unwrap_err(),
        ReconciliationError::UnsupportedSchema
    );
    assert_eq!(db.table_count().await, 0);
    db.cleanup().await;
    drop(_guard);
    for statement in [
        "UPDATE collaboration_identity_schema SET version=1",
        "ALTER TABLE collaboration_identity_audit DISABLE TRIGGER collaboration_identity_audit_immutable",
        "ALTER TABLE collaboration_policy_audit DISABLE TRIGGER collaboration_policy_audit_no_truncate",
        "ALTER TABLE users ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE collaboration_memberships ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE users INHERIT reconciliation_parent",
        "ALTER TABLE collaboration_identity_audit ALTER COLUMN recorded_at TYPE TEXT USING recorded_at::TEXT",
        "ALTER TABLE collaboration_policy_audit ALTER COLUMN after_state DROP NOT NULL",
        "ALTER TABLE sessions DROP CONSTRAINT sessions_pkey",
        "DROP TABLE collaboration_policy_audit",
    ] {
        let f = CheckFixture::new().await;
        if statement.contains(" INHERIT ") { f.db.sql("CREATE TABLE reconciliation_parent ()").await; }
        f.db.sql(statement).await;
        let before = if !statement.starts_with("DROP TABLE") { Some(f.digest().await) } else { None };
        assert_eq!(f.reconcile().await.unwrap_err(), ReconciliationError::UnsupportedSchema, "{statement}");
        if let Some(before) = before { assert_eq!(before, f.digest().await); }
        f.cleanup().await;
    }
    let f = CheckFixture::new().await;
    assert_eq!(
        f.db.source
            .reconcile_authority(WorkspaceRef {
                workspace_id: id(),
                ..scope()
            })
            .await
            .unwrap_err(),
        ReconciliationError::ScopeMismatch
    );
    let foreign = DurableAuthSource::new(f.db.pool.clone(), id());
    assert_eq!(
        foreign.reconcile_authority(scope()).await.unwrap_err(),
        ReconciliationError::ScopeMismatch
    );
    f.db.sql("UPDATE collaboration_auth_source_schema SET authority_id=gen_random_uuid()")
        .await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::ScopeMismatch
    );
    f.cleanup().await;
    let f = CheckFixture::new().await;
    let (_, bound) = f.bound("retired-renamed-source").await;
    CollaborationIdentityStore::new(f.db.pool.clone())
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.owner.identity.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: bound.binding.username.clone(),
                account_id: bound.account_id,
                expected_principal: bound.binding.principal_id,
            },
        })
        .await
        .unwrap();
    f.db.sql("ALTER TABLE users DISABLE TRIGGER USER").await;
    f.db.sql("UPDATE users SET username='different-name' WHERE username='retired-renamed-source'")
        .await;
    f.db.sql("ALTER TABLE users ENABLE TRIGGER USER").await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::InvalidSource,
        "retirement cannot permit account-ID reuse under another username"
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_missing_or_recreated_source_and_malformed_sessions_are_detected() {
    for recreate in [false, true] {
        let f = CheckFixture::new().await;
        f.db.sql("DELETE FROM users").await;
        if recreate {
            f.db.source.register(&username(), PASSWORD).await.unwrap();
        }
        let before = f.digest().await;
        assert_eq!(
            f.reconcile().await.unwrap_err(),
            ReconciliationError::InvalidSource
        );
        assert_eq!(before, f.digest().await);
        f.cleanup().await;
    }
    let f = CheckFixture::new().await;
    f.db.sql("ALTER TABLE sessions DROP CONSTRAINT collaboration_auth_session_digest")
        .await;
    f.db.sql("ALTER TABLE sessions ADD CONSTRAINT collaboration_auth_session_digest CHECK (TRUE)")
        .await;
    f.db.sql("INSERT INTO sessions (token, username, account_id, expires_at) SELECT 'not-a-digest', username, account_id, NOW()+interval '1 hour' FROM users").await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::InvalidSource
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_identity_and_policy_heads_must_match_current_rows() {
    for (statement, expected) in [
        (
            "UPDATE collaboration_principals SET display_name='unlogged rename'",
            ReconciliationError::IdentityHistoryMismatch,
        ),
        (
            "UPDATE collaboration_principals SET kind='agent'",
            ReconciliationError::InvalidIdentity,
        ),
        (
            "UPDATE collaboration_memberships SET status='suspended'",
            ReconciliationError::PolicyHistoryMismatch,
        ),
        (
            "UPDATE collaboration_deployment_grants SET revision=revision+1",
            ReconciliationError::InvalidPolicy,
        ),
        (
            "DELETE FROM collaboration_deployment_grants",
            ReconciliationError::InvalidPolicy,
        ),
    ] {
        let f = CheckFixture::new().await;
        f.db.sql(statement).await;
        let before = f.digest().await;
        assert_eq!(f.reconcile().await.unwrap_err(), expected, "{statement}");
        assert_eq!(before, f.digest().await);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_checks_entire_history_not_only_the_latest_receipt() {
    for identity in [true, false] {
        let f = CheckFixture::new().await;
        f.db.sql(
            "SELECT nextval(pg_get_serial_sequence('collaboration_identity_audit','sequence')),
            nextval(pg_get_serial_sequence('collaboration_policy_audit','sequence'))",
        )
        .await;
        let (_, bound) = f.bound("history-subject").await;
        let table = if identity {
            "collaboration_identity_audit"
        } else {
            "collaboration_policy_audit"
        };
        if identity {
            CollaborationIdentityStore::new(f.db.pool.clone())
                .apply_legacy_identity_command(&LegacyIdentityCommand {
                    command_id: id(),
                    actor: f.owner.identity.principal.reference,
                    workspace: scope(),
                    action: LegacyIdentityAction::Retire {
                        username: bound.binding.username.clone(),
                        account_id: bound.account_id,
                        expected_principal: bound.binding.principal_id,
                    },
                })
                .await
                .unwrap();
        } else {
            f.policy(bound.binding.principal_id, false).await;
            f.policy(bound.binding.principal_id, true).await;
        }
        assert!(f.reconcile().await.is_ok());
        f.db.sql(&format!("ALTER TABLE {table} DISABLE TRIGGER USER"))
            .await;
        let clause = if identity {
            "kind='bound'"
        } else {
            "kind='changed' AND revision=1"
        };
        f.db.sql(&format!(
            "UPDATE {table} SET request_digest=repeat('0',64) WHERE {clause}"
        ))
        .await;
        f.db.sql(&format!("ALTER TABLE {table} ENABLE TRIGGER USER"))
            .await;
        let expected = if identity {
            ReconciliationError::IdentityHistoryMismatch
        } else {
            ReconciliationError::PolicyHistoryMismatch
        };
        assert_eq!(
            f.reconcile().await.unwrap_err(),
            expected,
            "a corrupt earlier request is not hidden by a valid head"
        );
        f.db.sql(&format!("ALTER TABLE {table} DISABLE TRIGGER USER"))
            .await;
        f.db.sql(&format!("DELETE FROM {table} WHERE {clause}"))
            .await;
        f.db.sql(&format!("ALTER TABLE {table} ENABLE TRIGGER USER"))
            .await;
        assert_eq!(
            f.reconcile().await.unwrap_err(),
            if identity {
                ReconciliationError::IdentityHistoryMismatch
            } else {
                ReconciliationError::PolicyHistoryMismatch
            }
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_audit_payload_limits_and_missing_manager_never_report_success() {
    let f = CheckFixture::new().await;
    f.db.sql("INSERT INTO users (username, account_id, password_salt, password_hash, totp_secret)
        SELECT 'bounded-'||n, gen_random_uuid(), 'unused', 'unused', 'unused' FROM generate_series(1,10000) n").await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::LimitExceeded
    );
    assert_eq!(
        f.db.count("users").await,
        10001,
        "oversized input is not pruned"
    );
    f.cleanup().await;
    let f = CheckFixture::new().await;
    f.db.sql("ALTER TABLE collaboration_identity_audit DISABLE TRIGGER USER")
        .await;
    f.db.sql("UPDATE collaboration_identity_audit SET after_state=jsonb_set(after_state, '{principal,display_name}', to_jsonb(repeat('x', 20000)))").await;
    f.db.sql("ALTER TABLE collaboration_identity_audit ENABLE TRIGGER USER")
        .await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::LimitExceeded
    );
    f.cleanup().await;
    let f = CheckFixture::new().await;
    f.db.sql("INSERT INTO collaboration_principals (authority_id, principal_id, kind, display_name, status)
        SELECT authority_id, gen_random_uuid(), 'service', repeat('x',12000), 'active'
        FROM collaboration_authorities CROSS JOIN generate_series(1,400)").await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::LimitExceeded,
        "small individual rows must also fit the aggregate metadata budget"
    );
    f.cleanup().await;
    let f = CheckFixture::new().await;
    // A coherent observed baseline is not proof that a current source-backed manager remains.
    f.db.sql("ALTER TABLE collaboration_identity_audit DISABLE TRIGGER USER")
        .await;
    f.db.sql("UPDATE collaboration_principals SET status='disabled'")
        .await;
    f.db.sql("UPDATE collaboration_identity_audit SET after_state=jsonb_set(after_state, '{principal,status}', '\"disabled\"'::jsonb)").await;
    f.db.sql("ALTER TABLE collaboration_identity_audit ENABLE TRIGGER USER")
        .await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::NoCurrentManager
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_keeps_one_mvcc_snapshot_across_interleaved_commits() {
    let f = CheckFixture::new().await;
    let mut fence = f.db.pool.begin().await.unwrap();
    query("LOCK TABLE collaboration_policy_audit IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *fence)
        .await
        .unwrap();
    let source = f.db.source.clone();
    let pending = tokio::spawn(async move { source.reconcile_authority(scope()).await });
    f.db.wait_blocked().await;
    query("UPDATE collaboration_workspaces SET status='archived'")
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    let report = pending.await.unwrap().unwrap();
    assert_eq!(
        report.workspace_status,
        WorkspaceStatus::Active,
        "later queries must not switch to a newer snapshot"
    );
    assert_eq!(
        f.reconcile().await.unwrap().workspace_status,
        WorkspaceStatus::Archived
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_timeout_and_cancellation_publish_no_partial_result_or_write() {
    let f = CheckFixture::new().await;
    let before = f.digest().await;
    let mut fence = f.db.pool.begin().await.unwrap();
    query("LOCK TABLE users IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *fence)
        .await
        .unwrap();
    let source = f.db.source.clone();
    let pending = tokio::spawn(async move { source.reconcile_authority(scope()).await });
    f.db.wait_blocked().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::StorageUnavailable
    );
    fence.rollback().await.unwrap();
    f.db.drain().await;
    assert!(f.reconcile().await.is_ok());
    assert_eq!(before, f.digest().await);
    f.cleanup().await;
}
