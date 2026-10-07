use super::*;

const INVALID_TIMES: [&str; 3] = ["infinity", "-infinity", "262143-01-01 00:00:00+00"];

async fn snapshot(f: &Fixture) -> Vec<String> {
    let mut state = Vec::new();
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
        "collaboration_policy_audit",
    ] {
        // Keep complete synthetic rows, including corrupt dates, inside PostgreSQL. The test
        // snapshot must not itself invoke the binary chrono decoder under examination.
        state.push(
            query(&format!(
                "SELECT md5(COALESCE(string_agg(row::text, '' ORDER BY row::text), '')) AS digest
                FROM (SELECT to_jsonb(t) AS row FROM {table} t) s"
            ))
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("digest"),
        );
    }
    state
}

async fn set_recorded_at(f: &Fixture, sequence: u64, value: &str) {
    // Only fixture corruption bypasses immutability; restore it before calling any store API.
    f.sql("ALTER TABLE collaboration_policy_audit DISABLE TRIGGER collaboration_policy_audit_immutable").await;
    let changed = query(
        "UPDATE collaboration_policy_audit SET recorded_at = $2::text::timestamptz WHERE sequence = $1",
    )
    .bind(sequence as i64)
    .bind(value)
    .execute(&f.pool)
    .await;
    f.sql("ALTER TABLE collaboration_policy_audit ENABLE TRIGGER collaboration_policy_audit_immutable").await;
    assert_eq!(changed.unwrap().rows_affected(), 1);
}

fn rejected<T: std::fmt::Debug>(
    result: Result<Result<T, IdentityStoreError>, tokio::task::JoinError>,
    value: &str,
) {
    assert!(result.is_ok(), "{value}: a stored timestamp must not panic");
    assert_eq!(
        result.unwrap().unwrap_err(),
        IdentityStoreError::InvalidRecord
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_policy_audit_head_rejects_invalid_times_and_preserves_finite_boundaries() {
    for (value, expected_date) in [
        (INVALID_TIMES[0], None),
        (INVALID_TIMES[1], None),
        (INVALID_TIMES[2], None),
        ("4713-01-01 00:00:00+00 BC", Some((-4712, 1, 1))),
        ("262142-12-31 23:59:59.999999+00", Some((262142, 12, 31))),
    ] {
        let (f, manager, _, target) = ready().await;
        let expected_policy = current(&f, target).await;
        let baseline = history(&f, manager, target).await.remove(0);
        set_recorded_at(&f, baseline.sequence, value).await;
        let before = snapshot(&f).await;
        let store = f.store.clone();
        let lookup = tokio::spawn(async move { store.legacy_access(scope(), target).await }).await;
        let store = f.store.clone();
        let entries = tokio::spawn(async move {
            store
                .legacy_policy_audit(scope(), actor(manager), target, None, 100)
                .await
        })
        .await;
        f.drain().await;
        let after = snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(after, before, "{value}: reads must not mutate storage");
        if let Some((year, month, day)) = expected_date {
            assert_eq!(lookup.unwrap(), Ok(Some(expected_policy)));
            let entries = entries.unwrap().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].sequence, baseline.sequence);
            assert_eq!(
                entries[0].recorded_at.date_naive(),
                chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
            );
        } else {
            rejected(lookup, value);
            // Here history may reject its subject's head first; the next test isolates the
            // historical-row and replay decoders behind a healthy current head and actor.
            rejected(entries, value);
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_policy_audit_old_receipt_times_fail_history_and_replay_with_healthy_head() {
    for value in INVALID_TIMES {
        let (f, manager, _, target) = ready().await;
        let original = command(manager, target);
        let first = f
            .store
            .apply_legacy_policy_command(&original)
            .await
            .unwrap();
        let mut revoke = command(manager, target);
        revoke.expected_revision = Some(first.entry.after.revision);
        revoke.desired = policy(target, false);
        let latest = f.store.apply_legacy_policy_command(&revoke).await.unwrap();
        set_recorded_at(&f, first.entry.sequence, value).await;
        let before = snapshot(&f).await;
        let store = f.store.clone();
        let lookup = tokio::spawn(async move { store.legacy_access(scope(), target).await }).await;
        let store = f.store.clone();
        let actor_healthy = tokio::spawn(async move {
            store
                .preview_legacy_access(
                    scope(),
                    actor(manager),
                    LegacyAction::ManageDeployment,
                    &LegacyPolicyResource::Deployment {
                        authority_id: scope().authority_id,
                    },
                )
                .await
        })
        .await;
        let store = f.store.clone();
        let entries = tokio::spawn(async move {
            store
                .legacy_policy_audit(scope(), actor(manager), target, None, 100)
                .await
        })
        .await;
        let store = f.store.clone();
        let replay =
            tokio::spawn(async move { store.apply_legacy_policy_command(&original).await }).await;
        f.drain().await;
        let after = snapshot(&f).await;
        f.cleanup().await;
        assert!(first.entry.sequence < latest.entry.sequence);
        assert_eq!(lookup.unwrap(), Ok(Some(latest.entry.after)));
        assert_eq!(actor_healthy.unwrap(), Ok(true));
        rejected(entries, value);
        rejected(replay, value);
        assert_eq!(
            after, before,
            "{value}: rejection must not regrant or rewrite history"
        );
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_policy_audit_invalid_inserted_time_rolls_back_membership_grant_and_receipt() {
    for value in INVALID_TIMES {
        let (f, manager, _, target) = ready().await;
        f.sql("CREATE SEQUENCE policy_timestamp_calls").await;
        f.sql(&format!(
            "CREATE FUNCTION poison_policy_time() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.policy_timestamp_calls');
            NEW.recorded_at = '{value}'::timestamptz; RETURN NEW; END $$",
            f.schema
        ))
        .await;
        f.sql("CREATE TRIGGER poison_policy_time BEFORE INSERT ON collaboration_policy_audit FOR EACH ROW EXECUTE FUNCTION poison_policy_time()").await;
        let before = snapshot(&f).await;
        let store = f.store.clone();
        let pending = command(manager, target);
        let result =
            tokio::spawn(async move { store.apply_legacy_policy_command(&pending).await }).await;
        f.drain().await;
        let after = snapshot(&f).await;
        let hits: i64 = query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM policy_timestamp_calls")
            .fetch_one(&f.pool).await.unwrap().get("hits");
        f.cleanup().await;
        assert_eq!(
            hits, 1,
            "{value}: failure must reach receipt insertion after policy writes"
        );
        rejected(result, value);
        assert_eq!(
            after, before,
            "{value}: membership, grant and receipt must roll back together"
        );
    }
}
