use super::*;

const BAD_TIMES: [&str; 3] = ["infinity", "-infinity", "262143-01-01 00:00:00+00"];

async fn state(f: &Fixture) -> Vec<String> {
    let mut result = Vec::new();
    for table in [
        "collaboration_identity_schema",
        "collaboration_authorities",
        "collaboration_workspaces",
        "collaboration_legacy_workspaces",
        "collaboration_principals",
        "collaboration_legacy_identities",
    ] {
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

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_nullable_retirement_rejects_invalid_nonnull_times_without_reviving_accounts(
) {
    for timestamp in [
        None,
        Some("infinity"),
        Some("-infinity"),
        Some("262143-01-01 00:00:00+00"),
        Some("4713-01-01 00:00:00+00 BC"),
        Some("262142-12-31 23:59:59.999999+00"),
    ] {
        let f = Fixture::ready().await;
        let original = f.bind().await;
        let invalid = timestamp.is_some_and(|value| BAD_TIMES.contains(&value));
        if let Some(value) = timestamp {
            query("UPDATE collaboration_legacy_identities SET retired_at=$1::text::timestamptz")
                .bind(value)
                .execute(&f.pool)
                .await
                .unwrap();
            if !invalid {
                f.sql("UPDATE collaboration_principals SET status='disabled'")
                    .await;
            }
        }
        let before = state(&f).await;
        let store = f.store.clone();
        let lookup = tokio::spawn(async move {
            store
                .lookup_legacy_account(scope(), &username(), account())
                .await
        })
        .await;
        f.drain().await;
        let store = f.store.clone();
        let rebind = tokio::spawn(async move {
            store
                .bind_legacy_account(scope(), &username(), account())
                .await
        })
        .await;
        f.drain().await;
        let after = state(&f).await;
        f.cleanup().await;
        assert!(
            lookup.is_ok() && rebind.is_ok(),
            "{timestamp:?}: timestamp corruption must not panic"
        );
        let (lookup, rebind) = (lookup.unwrap(), rebind.unwrap());
        if invalid {
            assert_eq!(
                lookup,
                Err(IdentityStoreError::InvalidRecord),
                "{timestamp:?}"
            );
            assert_eq!(
                rebind,
                Err(IdentityStoreError::InvalidRecord),
                "{timestamp:?}"
            );
        } else if timestamp.is_none() {
            assert_eq!(lookup, Ok(Some(original.clone())));
            assert_eq!(rebind, Ok(original));
        } else {
            let observed = lookup.unwrap().unwrap();
            assert!(observed.retired_at.is_some());
            assert_eq!(observed.principal.status, PrincipalStatus::Disabled);
            assert_eq!(rebind, Err(IdentityStoreError::IdentityRetired));
        }
        assert_eq!(
            after, before,
            "{timestamp:?}: reads/rejections must not repair or revive state"
        );
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_retirement_trigger_invalid_time_rolls_back_binding_and_principal() {
    for timestamp in BAD_TIMES {
        let f = Fixture::ready().await;
        let original = f.bind().await;
        f.sql("CREATE SEQUENCE retirement_time_hits").await;
        f.sql(&format!("CREATE FUNCTION retirement_time_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.retirement_time_hits'); NEW.retired_at='{timestamp}'::timestamptz; RETURN NEW; END $$", f.schema)).await;
        f.sql(
            "CREATE TRIGGER retirement_time_fault BEFORE UPDATE ON collaboration_legacy_identities
            FOR EACH ROW EXECUTE FUNCTION retirement_time_fault()",
        )
        .await;
        let before = state(&f).await;
        let store = f.store.clone();
        let principal = original.binding.principal_id;
        let result = tokio::spawn(async move {
            store
                .retire_legacy_account(scope(), &username(), account(), principal)
                .await
        })
        .await;
        f.drain().await;
        let after = state(&f).await;
        let hits: i64 = query(
            "SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS n FROM retirement_time_hits",
        )
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("n");
        f.cleanup().await;
        assert_eq!(hits, 1, "{timestamp}: must reach the retirement UPDATE");
        assert!(result.is_ok(), "{timestamp}: write readback must not panic");
        assert_eq!(
            result.unwrap(),
            Err(IdentityStoreError::InvalidRecord),
            "{timestamp}"
        );
        assert_eq!(
            after, before,
            "{timestamp}: binding retirement and principal disable roll back together"
        );
    }
}
