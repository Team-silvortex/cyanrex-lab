use super::fixture::Fixture;
use cyanrex_engine::platform_host::PlatformHostPreflightError as Error;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_host_preflight_rejects_unsafe_runtime_flags_memberships_and_database_privileges()
{
    let (results, inherited_without_membership, restored) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let mut results = Vec::new();
            for (unsafe_flag, safe_flag) in [
                ("SUPERUSER", "NOSUPERUSER"),
                ("CREATEDB", "NOCREATEDB"),
                ("CREATEROLE", "NOCREATEROLE"),
                ("REPLICATION", "NOREPLICATION"),
                ("BYPASSRLS", "NOBYPASSRLS"),
                ("NOLOGIN", "LOGIN"),
            ] {
                fixture
                    .sql(&format!("ALTER ROLE {} {unsafe_flag}", fixture.role))
                    .await;
                results.push(fixture.inspect_unchanged().await);
                fixture
                    .sql(&format!("ALTER ROLE {} {safe_flag}", fixture.role))
                    .await;
            }
            let auxiliary = fixture.additional_role().await;
            fixture
                .sql(&format!("GRANT {auxiliary} TO {}", fixture.role))
                .await;
            results.push(fixture.inspect_unchanged().await);
            fixture
                .sql(&format!("REVOKE {auxiliary} FROM {}", fixture.role))
                .await;
            fixture
                .sql(&format!("ALTER ROLE {} INHERIT", fixture.role))
                .await;
            let inherited_without_membership = fixture.inspect_unchanged().await;
            fixture
                .sql(&format!("ALTER ROLE {} NOINHERIT", fixture.role))
                .await;

            let owner = fixture.owner_name().await.replace('"', "\"\"");
            fixture
                .sql(&format!(
                    "ALTER DATABASE {} OWNER TO {}",
                    fixture.database, fixture.role
                ))
                .await;
            results.push(fixture.inspect_unchanged().await);
            fixture
                .sql(&format!(
                    "ALTER DATABASE {} OWNER TO \"{owner}\"",
                    fixture.database
                ))
                .await;
            for privilege in ["CREATE", "TEMP"] {
                fixture
                    .sql(&format!(
                        "GRANT {privilege} ON DATABASE {} TO {}",
                        fixture.database, fixture.role
                    ))
                    .await;
                results.push(fixture.inspect_unchanged().await);
                fixture
                    .sql(&format!(
                        "REVOKE {privilege} ON DATABASE {} FROM {}",
                        fixture.database, fixture.role
                    ))
                    .await;
            }
            // An already-open caller connection remains usable after CONNECT is revoked;
            // inspect must still reject its current catalog privileges, not trust admission history.
            fixture
                .sql(&format!(
                    "REVOKE CONNECT ON DATABASE {} FROM {}",
                    fixture.database, fixture.role
                ))
                .await;
            results.push(fixture.inspect_unchanged().await);
            fixture
                .sql(&format!(
                    "GRANT CONNECT ON DATABASE {} TO {}",
                    fixture.database, fixture.role
                ))
                .await;
            let restored = fixture.inspect_unchanged().await;
            (results, inherited_without_membership, restored)
        })
    })
    .await;
    assert_eq!(results.len(), 11);
    for (result, unchanged) in results {
        assert_eq!(result, Err(Error::UnsafeRole));
        assert!(
            unchanged,
            "read-only rejection must not repair roles or grants"
        );
    }
    assert_eq!(inherited_without_membership.0, Ok(()));
    assert!(inherited_without_membership.1);
    assert_eq!(restored.0, Ok(()));
    assert!(restored.1);
}
