use super::{
    catalog_fixture::{within, Fixture},
    support::{attempt, config_variants, connector, mismatch, no_connections, Pins},
};
use cyanrex_engine::platform_host::PlatformDatabaseConnectionError;
use sqlx_core::{connection::Connection, query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_catalog_expectation_connects_fresh_and_checks_all_config_and_identity_fields(
) {
    let (
        created,
        opened,
        distinct,
        identities,
        mismatch_results,
        wrong_results,
        original_unchanged,
        catalog_unchanged,
        no_backend,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            // Get all expected OIDs independently before the first runtime-role inspection.
            let pins = Pins::read(fixture).await;
            let before = fixture.snapshot().await;
            let created = connector(fixture, pins);
            let created_ok = created.is_ok();
            let mut opened = 0;
            let mut distinct = false;
            let mut identities = false;
            if let Ok(connector) = created {
                let first = within(connector.connect()).await;
                let second = within(connector.connect()).await;
                opened = usize::from(first.is_ok()) + usize::from(second.is_ok());
                match (first, second) {
                    (Ok(mut first), Ok(mut second)) => {
                        let statement = "SELECT pg_catalog.pg_backend_pid() AS pid,
                            current_user::text=$1 AND session_user::text=$1 AS exact_role,
                            current_database()::text=$2 AS exact_database";
                        let a = within(
                            query(statement)
                                .bind(&fixture.role)
                                .bind(&fixture.database)
                                .fetch_one(&mut first),
                        )
                        .await
                        .unwrap();
                        let b = within(
                            query(statement)
                                .bind(&fixture.role)
                                .bind(&fixture.database)
                                .fetch_one(&mut second),
                        )
                        .await
                        .unwrap();
                        distinct = a.get::<i32, _>("pid") != b.get::<i32, _>("pid");
                        identities = a.get::<bool, _>("exact_role")
                            && b.get::<bool, _>("exact_role")
                            && a.get::<bool, _>("exact_database")
                            && b.get::<bool, _>("exact_database");
                        within(first.close()).await.unwrap();
                        within(second.close()).await.unwrap();
                    }
                    (first, second) => {
                        if let Ok(connection) = first {
                            within(connection.close()).await.unwrap();
                        }
                        if let Ok(connection) = second {
                            within(connection.close()).await.unwrap();
                        }
                    }
                }
            }
            let mismatch_results = config_variants(fixture)
                .into_iter()
                .map(|config| mismatch(fixture, pins, config))
                .collect::<Vec<_>>();
            let mut wrong_results = Vec::new();
            for index in 0..8 {
                let wrong = connector(fixture, pins.wrong(index)).unwrap_or_else(|_| {
                    panic!("syntactically valid wrong expectation must be constructible")
                });
                wrong_results.push(attempt(&wrong).await);
            }
            let original_unchanged = Pins::read(fixture).await == pins;
            let catalog_unchanged = before == fixture.snapshot().await;
            let no_backend = no_connections(fixture).await;
            (
                created_ok,
                opened,
                distinct,
                identities,
                mismatch_results,
                wrong_results,
                original_unchanged,
                catalog_unchanged,
                no_backend,
            )
        })
    })
    .await;
    // Every runtime connection, owned database/role and fixture pool is cleaned first.
    assert!(created);
    assert_eq!(opened, 2);
    assert!(distinct && identities);
    assert_eq!(mismatch_results.len(), 12);
    assert!(mismatch_results
        .into_iter()
        .all(|result| result == Err(PlatformDatabaseConnectionError::InvalidConfig)));
    assert_eq!(wrong_results.len(), 8);
    assert!(wrong_results
        .into_iter()
        .all(|result| result == Err(PlatformDatabaseConnectionError::Unavailable)));
    assert!(original_unchanged && catalog_unchanged && no_backend);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_catalog_expectation_rejects_same_name_namespace_replacement_before_first_connect(
) {
    let (
        created,
        rejected,
        independently_valid,
        replacement_changed,
        restored,
        unchanged,
        no_backend,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            let pins = Pins::read(fixture).await;
            let before = fixture.snapshot().await;
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
            let replacement_changed = Pins::read(fixture).await != pins;
            // A current observer accepts this otherwise valid replacement. It must
            // not become the baseline for the independently declared original OIDs.
            let independently_valid = fixture.observe().await.is_ok();
            let created = connector(fixture, pins);
            let created_ok = created.is_ok();
            let rejected = match &created {
                Ok(connector) => attempt(connector).await,
                Err(error) => Err(*error),
            };
            fixture.sql(&format!("DROP SCHEMA {}", fixture.task)).await;
            fixture
                .sql(&format!("ALTER SCHEMA {saved} RENAME TO {}", fixture.task))
                .await;
            let restored = match &created {
                Ok(connector) => attempt(connector).await,
                Err(error) => Err(*error),
            };
            let unchanged = Pins::read(fixture).await == pins && before == fixture.snapshot().await;
            let no_backend = no_connections(fixture).await;
            (
                created_ok,
                rejected,
                independently_valid,
                replacement_changed,
                restored,
                unchanged,
                no_backend,
            )
        })
    })
    .await;
    assert!(
        created,
        "declaration construction must not dial or adopt a current observation"
    );
    assert!(independently_valid && replacement_changed);
    assert_eq!(rejected, Err(PlatformDatabaseConnectionError::Unavailable));
    assert_eq!(restored, Ok(()));
    assert!(unchanged && no_backend);
}
