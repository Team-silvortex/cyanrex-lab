use super::*;

const BAD_TIMES: [&str; 3] = ["infinity", "-infinity", "262143-01-01 00:00:00+00"];

async fn advance_audit_heads(f: &CheckFixture) {
    let registry = CollaborationIdentityStore::new(f.db.pool.clone());
    registry
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.owner.identity.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Bind {
                username: username(),
                account_id: f.owner.registration.account.account_id,
            },
        })
        .await
        .unwrap();
    let policy = registry
        .legacy_access(scope(), f.owner.identity.binding.principal_id)
        .await
        .unwrap()
        .unwrap();
    registry
        .apply_legacy_policy_command(
            &cyanrex_engine::services::collaboration_identity_store::LegacyPolicyCommand {
                command_id: id(),
                actor: f.owner.identity.principal.reference,
                expected_revision: Some(policy.revision),
                desired: policy.policy,
            },
        )
        .await
        .unwrap();
}

async fn set_time(f: &CheckFixture, kind: &str, value: &str) {
    let table = match kind {
        "retirement" => {
            query("UPDATE collaboration_legacy_identities SET retired_at=$1::text::timestamptz")
                .bind(value)
                .execute(&f.db.pool)
                .await
                .unwrap();
            return;
        }
        "identity" => "collaboration_identity_audit",
        "policy" => "collaboration_policy_audit",
        _ => unreachable!(),
    };
    // Poison the oldest record; the latest head stays healthy. Restore audit protection
    // BEFORE testing so refusal cannot be an unrelated disabled-trigger schema failure.
    f.db.sql(&format!("ALTER TABLE {table} DISABLE TRIGGER USER"))
        .await;
    query(&format!(
        "UPDATE {table} SET recorded_at=$1::text::timestamptz
        WHERE sequence=(SELECT min(sequence) FROM {table})"
    ))
    .bind(value)
    .execute(&f.db.pool)
    .await
    .unwrap();
    f.db.sql(&format!("ALTER TABLE {table} ENABLE TRIGGER USER"))
        .await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_invalid_registry_times_reject_their_exact_stage_without_writing() {
    for (kind, expected) in [
        ("retirement", ReconciliationError::InvalidIdentity),
        ("identity", ReconciliationError::IdentityHistoryMismatch),
        ("policy", ReconciliationError::PolicyHistoryMismatch),
    ] {
        for value in BAD_TIMES {
            let f = CheckFixture::new().await;
            advance_audit_heads(&f).await;
            set_time(&f, kind, value).await;
            let before = f.digest().await;
            let source = f.db.source.clone();
            let result =
                tokio::spawn(async move { source.reconcile_authority(scope()).await }).await;
            f.db.drain().await;
            let after = f.digest().await;
            f.cleanup().await;
            assert_eq!(before, after, "{kind}: {value}");
            assert_eq!(
                result
                    .expect("bad registry time must not panic")
                    .unwrap_err(),
                expected,
                "{kind}: {value}"
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_finite_audit_times_preserve_sequence_based_history() {
    let f = CheckFixture::new().await;
    advance_audit_heads(&f).await;
    let mut observed = Vec::new();
    for kind in ["identity", "policy"] {
        for value in [
            "4713-01-01 00:00:00+00 BC",
            "262142-12-31 23:59:59.999999+00",
        ] {
            set_time(&f, kind, value).await;
            let before = f.digest().await;
            let source = f.db.source.clone();
            let result =
                tokio::spawn(async move { source.reconcile_authority(scope()).await }).await;
            f.db.drain().await;
            let after = f.digest().await;
            observed.push((kind, value, before, after, result));
        }
    }
    f.cleanup().await;
    for (kind, value, before, after, result) in observed {
        assert_eq!(before, after, "{kind}: {value}");
        let report = result.expect("finite audit time must not panic").unwrap();
        assert_eq!(report.identity_audit_entries, 2);
        assert_eq!(report.policy_audit_entries, 2);
        assert_eq!(report.current_managers, 1);
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_session_command_bad_registry_times_cannot_authorize_a_binding() {
    use cyanrex_engine::services::{
        auth_service::durable_source::SessionCommandError,
        collaboration_identity_store::IdentityStoreError,
    };
    for kind in ["retirement", "identity", "policy"] {
        for value in BAD_TIMES {
            let f = CheckFixture::new().await;
            let token = f.owner_login().await;
            let target =
                f.db.source
                    .register(&"timestamp-target".parse().unwrap(), PASSWORD)
                    .await
                    .unwrap();
            // One baseline is the actor's current head, so authorization must see this fault.
            set_time(&f, kind, value).await;
            let before = f.digest().await;
            let source = f.db.source.clone();
            let pending = tokio::spawn(async move {
                source
                    .bind_session_account(
                        &token,
                        &SessionBindCommand {
                            command_id: id(),
                            workspace: scope(),
                            target: target.account,
                        },
                    )
                    .await
            })
            .await;
            f.db.drain().await;
            let after = f.digest().await;
            f.cleanup().await;
            assert_eq!(before, after, "{kind}: {value}");
            assert_eq!(
                pending
                    .expect("invalid actor time must not panic")
                    .unwrap_err(),
                SessionCommandError::Identity(IdentityStoreError::InvalidRecord),
                "{kind}: {value}"
            );
        }
    }
}
