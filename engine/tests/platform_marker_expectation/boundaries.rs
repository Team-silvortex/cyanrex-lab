use super::{
    catalog_fixture::{within, Fixture},
    expectation_helpers::{config_variants, quote_identifier, Pins},
    marker_fixture::{prepare, snapshot},
    support::{current, namespace, no_connections, observe, relation, MarkerPins, TABLES},
};
use cyanrex_engine::platform_host::{
    inspect_platform_storage_markers_expected, PlatformHostPreflightError,
};
use sqlx_core::{connection::Connection, executor::Executor, query::query, row::Row};

async fn replace(fixture: &Fixture, index: usize) {
    let relation = relation(fixture, index);
    let saved = format!("{}_expectation_saved", TABLES[index]);
    fixture
        .sql(&format!("ALTER TABLE {relation} RENAME TO {saved}"))
        .await;
    let scope = if index == 0 {
        ",authority_id uuid NOT NULL"
    } else if index >= 3 {
        ",authority_id uuid NOT NULL,workspace_id uuid NOT NULL"
    } else {
        ""
    };
    fixture.sql(&format!("CREATE TABLE {relation}(singleton boolean NOT NULL PRIMARY KEY,version integer NOT NULL{scope})")).await;
    let version = match index {
        0..=2 => 2,
        3 => 3,
        4 => 1,
        _ => unreachable!(),
    };
    let values = if index == 0 {
        format!(",'{}'", fixture.config().scope().authority_id.as_uuid())
    } else if index >= 3 {
        format!(
            ",'{}','{}'",
            fixture.config().scope().authority_id.as_uuid(),
            fixture.config().scope().workspace_id.as_uuid()
        )
    } else {
        String::new()
    };
    fixture
        .sql(&format!(
            "INSERT INTO {relation} VALUES(TRUE,{version}{values})"
        ))
        .await;
    fixture
        .sql(&format!("GRANT SELECT ON {relation} TO {}", fixture.role))
        .await;
}

async fn restore(fixture: &Fixture, index: usize) {
    fixture
        .sql(&format!("DROP TABLE {}", relation(fixture, index)))
        .await;
    fixture
        .sql(&format!(
            "ALTER TABLE {}.{}_expectation_saved RENAME TO {}",
            namespace(fixture, index),
            TABLES[index],
            TABLES[index]
        ))
        .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_marker_expectation_checks_all_identity_slots_and_five_first_call_replacements(
) {
    let (
        wrong_markers,
        wrong_catalogs,
        wrong_configs,
        replaced,
        positive,
        recovered,
        ordering,
        body_control,
        no_repair,
        unchanged,
        no_backend,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let catalog = Pins::read(fixture).await;
            let markers = MarkerPins::read(fixture).await;
            let before = snapshot(fixture).await;
            let mut wrong_markers = Vec::new();
            for index in 0..10 {
                wrong_markers.push(
                    observe(fixture, &markers.wrong(index).expectation(fixture, catalog))
                        .await
                        .map(|_| ()),
                );
            }
            let mut wrong_catalogs = Vec::new();
            for index in 0..8 {
                wrong_catalogs.push(
                    observe(fixture, &markers.expectation(fixture, catalog.wrong(index)))
                        .await
                        .map(|_| ()),
                );
            }
            let expected = markers.expectation(fixture, catalog);
            let mut wrong_configs = Vec::new();
            let mut connection = within(fixture.runtime().acquire()).await.unwrap();
            for config in config_variants(fixture) {
                wrong_configs.push(
                    within(inspect_platform_storage_markers_expected(
                        &config,
                        &mut connection,
                        &expected,
                    ))
                    .await
                    .map(|_| ()),
                );
            }
            drop(connection);
            let mut permuted = markers;
            permuted.oids.swap(0, 4);
            let identity_order = observe(fixture, &permuted.expectation(fixture, catalog))
                .await
                .map(|_| ())
                == Err(PlatformHostPreflightError::StorageMarkersChanged);
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=0 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let bad_body = snapshot(fixture).await;
            // The last identity is wrong while the first body is independently invalid.
            // Array identity rejection must precede every body read/error classification.
            let ordering = observe(fixture, &markers.wrong(4).expectation(fixture, catalog))
                .await
                .map(|_| ());
            let body_control = current(fixture).await.map(|_| ());
            let body_not_repaired = bad_body == snapshot(fixture).await;
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=2 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let mut replaced = Vec::new();
            let mut positive = identity_order;
            let mut recovered = true;
            let mut no_repair = true;
            for index in 0..5 {
                // A new immutable declaration has never been inspected before replacement.
                let original = markers.expectation(fixture, catalog);
                replace(fixture, index).await;
                let changed = snapshot(fixture).await;
                let changed_pins = MarkerPins::read(fixture).await;
                positive &= changed_pins.oids[index] != markers.oids[index]
                    && current(fixture).await.is_ok();
                replaced.push(observe(fixture, &original).await.map(|_| ()));
                no_repair &= changed == snapshot(fixture).await
                    && changed_pins == MarkerPins::read(fixture).await;
                restore(fixture, index).await;
                recovered &= observe(fixture, &original).await.is_ok();
            }
            let unchanged = before == snapshot(fixture).await
                && catalog == Pins::read(fixture).await
                && markers == MarkerPins::read(fixture).await;
            let no_backend = no_connections(fixture).await;
            (
                wrong_markers,
                wrong_catalogs,
                wrong_configs,
                replaced,
                positive,
                recovered,
                ordering,
                body_control,
                no_repair && body_not_repaired,
                unchanged,
                no_backend,
            )
        })
    })
    .await;
    assert_eq!(wrong_markers.len(), 10);
    assert_eq!(wrong_catalogs.len(), 8);
    assert_eq!(wrong_configs.len(), 12);
    assert_eq!(replaced.len(), 5);
    for result in wrong_markers.into_iter().chain(replaced) {
        assert_eq!(
            result,
            Err(PlatformHostPreflightError::StorageMarkersChanged)
        );
    }
    for result in wrong_catalogs.into_iter().chain(wrong_configs) {
        assert_eq!(result, Err(PlatformHostPreflightError::CatalogChanged));
    }
    assert_eq!(
        ordering,
        Err(PlatformHostPreflightError::StorageMarkersChanged)
    );
    assert_eq!(
        body_control,
        Err(PlatformHostPreflightError::UnsupportedStorageMarker)
    );
    assert!(positive && recovered && no_repair && unchanged && no_backend);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_marker_expectation_rejects_owner_drift_and_preserves_outer_transactions()
{
    let (
        owner_error,
        positive,
        restored,
        managed,
        managed_preserved,
        raw,
        raw_preserved,
        unchanged,
        no_backend,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let auxiliary = fixture.additional_role().await;
            let owner = quote_identifier(&fixture.owner_name().await);
            let catalog = Pins::read(fixture).await;
            let markers = MarkerPins::read(fixture).await;
            let expected = markers.expectation(fixture, catalog);
            let before = snapshot(fixture).await;
            fixture
                .sql(&format!(
                    "ALTER TABLE {} OWNER TO {auxiliary}",
                    relation(fixture, 0)
                ))
                .await;
            let positive = current(fixture).await.is_ok();
            let owner_error = observe(fixture, &expected).await.map(|_| ());
            fixture
                .sql(&format!(
                    "ALTER TABLE {} OWNER TO {owner}",
                    relation(fixture, 0)
                ))
                .await;
            let restored = observe(fixture, &expected).await.map(|_| ());
            let mut connection = within(fixture.runtime().acquire()).await.unwrap();
            let mut outer = within(connection.begin()).await.unwrap();
            within(query("SET LOCAL lock_timeout='777ms'").execute(&mut *outer))
                .await
                .unwrap();
            let managed = within(inspect_platform_storage_markers_expected(
                fixture.config(),
                &mut outer,
                &expected,
            ))
            .await
            .map(|_| ());
            let managed_preserved = within(
                query("SELECT current_setting('lock_timeout')='777ms' AS preserved")
                    .fetch_one(&mut *outer),
            )
            .await
            .is_ok_and(|row| row.get::<bool, _>("preserved"));
            within(outer.rollback()).await.unwrap();
            Executor::execute(
                &mut *connection,
                "BEGIN; SET LOCAL lock_timeout='777ms'; SAVEPOINT caller_marker_expectation",
            )
            .await
            .unwrap();
            let raw = within(inspect_platform_storage_markers_expected(
                fixture.config(),
                &mut connection,
                &expected,
            ))
            .await
            .map(|_| ());
            let raw_preserved = Executor::execute(
                &mut *connection,
                "ROLLBACK TO SAVEPOINT caller_marker_expectation",
            )
            .await
            .is_ok()
                && within(
                    query("SELECT current_setting('lock_timeout')='777ms' AS preserved")
                        .fetch_one(&mut *connection),
                )
                .await
                .is_ok_and(|row| row.get::<bool, _>("preserved"));
            Executor::execute(&mut *connection, "ROLLBACK")
                .await
                .unwrap();
            drop(connection);
            let unchanged = before == snapshot(fixture).await
                && catalog == Pins::read(fixture).await
                && markers == MarkerPins::read(fixture).await;
            let no_backend = no_connections(fixture).await;
            (
                owner_error,
                positive,
                restored,
                managed,
                managed_preserved,
                raw,
                raw_preserved,
                unchanged,
                no_backend,
            )
        })
    })
    .await;
    assert!(positive);
    assert_eq!(
        owner_error,
        Err(PlatformHostPreflightError::StorageMarkersChanged)
    );
    assert_eq!(restored, Ok(()));
    assert_eq!(managed, Err(PlatformHostPreflightError::InvalidConfig));
    assert_eq!(raw, Err(PlatformHostPreflightError::InvalidConfig));
    assert!(managed_preserved && raw_preserved && unchanged && no_backend);
}
