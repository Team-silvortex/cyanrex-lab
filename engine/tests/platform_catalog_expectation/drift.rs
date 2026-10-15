use super::{
    catalog_fixture::Fixture,
    support::{
        attempt, connector, no_connections, prepare_replacement_role, quote_identifier, swap_roles,
        Pins,
    },
};
use cyanrex_engine::platform_host::PlatformDatabaseConnectionError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_catalog_expectation_rechecks_role_namespace_owner_and_privilege_drift() {
    let (first, rejected, role_replaced, restored, no_repair, unchanged, no_backend) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let auxiliary = prepare_replacement_role(fixture).await;
                // Baseline the runtime entry last: revoke/regrant below then restores the exact
                // ACL array order, not just its privilege set. This is fixture setup only.
                fixture
                    .sql(&format!(
                        "REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {}",
                        fixture.role
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {}",
                        fixture.role
                    ))
                    .await;
                let pins = Pins::read(fixture).await;
                let before = fixture.snapshot().await;
                let connector = connector(fixture, pins).unwrap();
                let first = attempt(&connector).await;
                let mut rejected = Vec::new();
                let mut no_repair = true;
                let owner = quote_identifier(&fixture.owner_name().await);

                fixture
                    .sql(&format!(
                        "ALTER SCHEMA {} OWNER TO {auxiliary}",
                        fixture.task
                    ))
                    .await;
                let changed = fixture.snapshot().await;
                rejected.push(attempt(&connector).await);
                no_repair &= changed == fixture.snapshot().await;
                fixture
                    .sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", fixture.task))
                    .await;
                fixture
                    .sql(&format!(
                        "GRANT USAGE ON SCHEMA {} TO {auxiliary}",
                        fixture.task
                    ))
                    .await;

                let swapped = swap_roles(fixture, &auxiliary).await;
                let role_replaced = swapped && Pins::read(fixture).await.role != pins.role;
                let changed = fixture.snapshot().await;
                rejected.push(attempt(&connector).await);
                no_repair &= changed == fixture.snapshot().await;
                let swapped_back = !swapped || swap_roles(fixture, &auxiliary).await;
                no_repair &= swapped_back;

                let saved = format!("{}_saved", fixture.task);
                fixture
                    .sql(&format!("ALTER SCHEMA {} RENAME TO {saved}", fixture.task))
                    .await;
                fixture
                    .sql(&format!("CREATE SCHEMA {}", fixture.task))
                    .await;
                fixture
                    .sql(&format!(
                        "REVOKE ALL ON SCHEMA {} FROM PUBLIC",
                        fixture.task
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "GRANT USAGE ON SCHEMA {} TO {}",
                        fixture.task, fixture.role
                    ))
                    .await;
                let changed = fixture.snapshot().await;
                rejected.push(attempt(&connector).await);
                no_repair &= changed == fixture.snapshot().await;
                fixture.sql(&format!("DROP SCHEMA {}", fixture.task)).await;
                fixture
                    .sql(&format!("ALTER SCHEMA {saved} RENAME TO {}", fixture.task))
                    .await;

                for (change, restore) in [
                    (
                        format!(
                            "GRANT CREATE ON SCHEMA {} TO {}",
                            fixture.source, fixture.role
                        ),
                        format!(
                            "REVOKE CREATE ON SCHEMA {} FROM {}",
                            fixture.source, fixture.role
                        ),
                    ),
                    (
                        format!(
                            "GRANT TEMP ON DATABASE {} TO {}",
                            fixture.database, fixture.role
                        ),
                        format!(
                            "REVOKE TEMP ON DATABASE {} FROM {}",
                            fixture.database, fixture.role
                        ),
                    ),
                    (
                        format!(
                            "REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {}",
                            fixture.role
                        ),
                        format!(
                            "GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {}",
                            fixture.role
                        ),
                    ),
                ] {
                    fixture.sql(&change).await;
                    let changed = fixture.snapshot().await;
                    rejected.push(attempt(&connector).await);
                    no_repair &= changed == fixture.snapshot().await;
                    fixture.sql(&restore).await;
                }
                let restored = attempt(&connector).await;
                let unchanged =
                    before == fixture.snapshot().await && Pins::read(fixture).await == pins;
                let no_backend = no_connections(fixture).await;
                (
                    first,
                    rejected,
                    role_replaced,
                    restored,
                    no_repair,
                    unchanged,
                    no_backend,
                )
            })
        })
        .await;
    assert_eq!(first, Ok(()));
    assert!(role_replaced);
    assert_eq!(rejected.len(), 6);
    assert!(rejected
        .into_iter()
        .all(|value| value == Err(PlatformDatabaseConnectionError::Unavailable)));
    assert_eq!(restored, Ok(()));
    assert!(
        no_repair,
        "verification must not repair or mutate the drifted catalog"
    );
    assert!(
        unchanged,
        "fixture restoration must recover the original catalog"
    );
    assert!(no_backend, "every unpublished connection must close");
}
