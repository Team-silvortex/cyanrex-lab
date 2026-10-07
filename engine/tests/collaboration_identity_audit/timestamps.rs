use super::*;

const BAD_TIMES: [&str; 3] = ["infinity", "-infinity", "262143-01-01 00:00:00+00"];

async fn state(f: &Fixture) -> Vec<String> {
    let mut result = Vec::new();
    for table in [
        "collaboration_identity_schema",
        "collaboration_access_schema",
        "collaboration_authorities",
        "collaboration_workspaces",
        "collaboration_legacy_workspaces",
        "collaboration_principals",
        "collaboration_legacy_identities",
        "collaboration_memberships",
        "collaboration_deployment_grants",
        "collaboration_identity_audit",
        "collaboration_policy_audit",
    ] {
        // Hash complete rows inside PostgreSQL; no corrupt timestamp is decoded just to snapshot it.
        result.push(
            query(&format!(
                "SELECT COALESCE(string_agg(md5(row_to_json(t)::text), ','
            ORDER BY md5(row_to_json(t)::text)), '') AS state FROM {table} t"
            ))
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("state"),
        );
    }
    result
}

async fn poison_receipt(f: &Fixture, command: IdentityCommandId, timestamp: &str) {
    f.sql("ALTER TABLE collaboration_identity_audit DISABLE TRIGGER collaboration_identity_audit_immutable").await;
    query("UPDATE collaboration_identity_audit SET recorded_at=$1::text::timestamptz WHERE command_id=$2")
        .bind(timestamp).bind(command.as_uuid()).execute(&f.pool).await.unwrap();
    f.sql("ALTER TABLE collaboration_identity_audit ENABLE TRIGGER collaboration_identity_audit_immutable").await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_timestamp_reads_cover_nullable_retirement_head_history_and_old_replay()
{
    // The valid audit head still says active + NULL. A SQL CASE that maps a bad non-NULL retirement
    // to None without its validity flag could therefore incorrectly pass the head equality check.
    for timestamp in BAD_TIMES {
        let (f, _, _, target) = ready().await;
        let store = f.store.clone();
        let account = target.account_id;
        let good = tokio::spawn(async move {
            store
                .lookup_legacy_account(scope(), &username(), account)
                .await
        })
        .await;
        f.drain().await;
        query("UPDATE collaboration_legacy_identities SET retired_at=$1::text::timestamptz WHERE principal_id=$2")
            .bind(timestamp).bind(target.binding.principal_id.as_uuid()).execute(&f.pool).await.unwrap();
        let before = state(&f).await;
        let store = f.store.clone();
        let bad = tokio::spawn(async move {
            store
                .lookup_legacy_account(scope(), &username(), account)
                .await
        })
        .await;
        f.drain().await;
        let after = state(&f).await;
        f.cleanup().await;
        assert_eq!(good.unwrap(), Ok(Some(target)));
        assert!(
            bad.is_ok(),
            "{timestamp}: nullable retirement must not panic"
        );
        assert_eq!(
            bad.unwrap(),
            Err(IdentityStoreError::InvalidRecord),
            "{timestamp}"
        );
        assert_eq!(after, before);
    }
    for latest in [false, true] {
        for timestamp in BAD_TIMES {
            let (f, manager, _, _) = ready().await;
            let manager = manager.binding.principal_id;
            let old = bind(manager);
            let applied = f.store.apply_legacy_identity_command(&old).await.unwrap();
            let mut newer = old.clone();
            newer.command_id = id();
            let head = f.store.apply_legacy_identity_command(&newer).await.unwrap();
            let selected = if latest { newer } else { old };
            poison_receipt(&f, selected.command_id, timestamp).await;
            let before = state(&f).await;
            let subject = applied.entry.after.binding.principal_id;
            let name = applied.entry.after.binding.username.clone();
            let account = applied.entry.after.account_id;
            let store = f.store.clone();
            let lookup =
                tokio::spawn(
                    async move { store.lookup_legacy_account(scope(), &name, account).await },
                )
                .await;
            f.drain().await;
            let store = f.store.clone();
            let history = tokio::spawn(async move {
                store
                    .legacy_identity_audit(scope(), actor(manager), subject, None, 100)
                    .await
            })
            .await;
            f.drain().await;
            let store = f.store.clone();
            let replay =
                tokio::spawn(async move { store.apply_legacy_identity_command(&selected).await })
                    .await;
            f.drain().await;
            let after = state(&f).await;
            f.cleanup().await;
            assert!(
                lookup.is_ok() && history.is_ok() && replay.is_ok(),
                "latest={latest} {timestamp}: audit decoding must not panic"
            );
            if latest {
                assert_eq!(lookup.unwrap(), Err(IdentityStoreError::InvalidRecord));
            } else {
                assert_eq!(
                    lookup.unwrap(),
                    Ok(Some(head.entry.after)),
                    "healthy latest head must remain readable before the old-history failure"
                );
            }
            assert_eq!(history.unwrap(), Err(IdentityStoreError::InvalidRecord));
            assert_eq!(replay.unwrap(), Err(IdentityStoreError::InvalidRecord));
            assert_eq!(after, before, "latest={latest} {timestamp}");
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_audit_append_invalid_time_rolls_back_binding_and_retirement() {
    for retirement in [false, true] {
        for timestamp in BAD_TIMES {
            let (f, manager, _, target) = ready().await;
            let command = if retirement {
                retire(manager.binding.principal_id, &target)
            } else {
                bind(manager.binding.principal_id)
            };
            f.sql("CREATE SEQUENCE identity_time_hits").await;
            f.sql(&format!("CREATE FUNCTION identity_time_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM nextval('{}.identity_time_hits'); NEW.recorded_at='{timestamp}'::timestamptz; RETURN NEW; END $$", f.schema)).await;
            f.sql(
                "CREATE TRIGGER identity_time_fault BEFORE INSERT ON collaboration_identity_audit
                FOR EACH ROW EXECUTE FUNCTION identity_time_fault()",
            )
            .await;
            let before = state(&f).await;
            let store = f.store.clone();
            let result =
                tokio::spawn(async move { store.apply_legacy_identity_command(&command).await })
                    .await;
            f.drain().await;
            let after = state(&f).await;
            let hits: i64 = query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS n FROM identity_time_hits")
                .fetch_one(&f.pool).await.unwrap().get("n");
            f.cleanup().await;
            assert_eq!(
                hits, 1,
                "retirement={retirement} {timestamp}: append must be reached"
            );
            assert!(
                result.is_ok(),
                "retirement={retirement} {timestamp}: append readback must not panic"
            );
            assert_eq!(result.unwrap(), Err(IdentityStoreError::InvalidRecord));
            assert_eq!(
                after, before,
                "retirement={retirement} {timestamp}: state and receipt must roll back together"
            );
        }
    }
}
