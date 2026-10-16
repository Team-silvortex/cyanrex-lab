use super::{
    catalog_fixture::Fixture,
    expectation_helpers::Pins,
    marker_fixture::{prepare, snapshot},
    support::{endpoint, no_connections, relation, MarkerPins, PASSWORD},
};
use cyanrex_engine::platform_host::{
    check_platform_database_storage_markers_expected, PlatformDatabaseEndpoint,
    PlatformHostPreflightError,
};
use std::{
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_marker_expectation_diagnostic_uses_one_fresh_explicit_attempt_and_closes(
) {
    let (
        success,
        bad_password,
        bad_marker,
        bad_version,
        restored,
        accepted,
        probe,
        no_retry,
        unchanged,
        no_backend,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let catalog = Pins::read(fixture).await;
            let markers = MarkerPins::read(fixture).await;
            let expected = markers.expectation(fixture, catalog);
            let transport = endpoint(fixture);
            let before = snapshot(fixture).await;
            let success = check_platform_database_storage_markers_expected(
                fixture.config(),
                &transport,
                PASSWORD,
                &expected,
            )
            .await;
            let bad_password = check_platform_database_storage_markers_expected(
                fixture.config(),
                &transport,
                "synthetic-wrong-password",
                &expected,
            )
            .await;
            let bad_marker = check_platform_database_storage_markers_expected(
                fixture.config(),
                &transport,
                PASSWORD,
                &markers.wrong(3).expectation(fixture, catalog),
            )
            .await;
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=99 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let changed = snapshot(fixture).await;
            let bad_version = check_platform_database_storage_markers_expected(
                fixture.config(),
                &transport,
                PASSWORD,
                &expected,
            )
            .await;
            let not_repaired = changed == snapshot(fixture).await;
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=2 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let restored = check_platform_database_storage_markers_expected(
                fixture.config(),
                &transport,
                PASSWORD,
                &expected,
            )
            .await;
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let probe_endpoint = PlatformDatabaseEndpoint::loopback(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                listener.local_addr().unwrap().port(),
            )
            .unwrap();
            let (probe, accepted) = tokio::join!(
                check_platform_database_storage_markers_expected(
                    fixture.config(),
                    &probe_endpoint,
                    PASSWORD,
                    &expected
                ),
                async {
                    match tokio::time::timeout(Duration::from_secs(2), listener.accept()).await {
                        Ok(Ok((socket, _))) => {
                            drop(socket);
                            true
                        }
                        _ => false,
                    }
                }
            );
            let no_retry =
                match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
                    Err(_) => true,
                    Ok(Ok((socket, _))) => {
                        drop(socket);
                        false
                    }
                    _ => false,
                };
            drop(listener);
            let unchanged = not_repaired
                && before == snapshot(fixture).await
                && markers == MarkerPins::read(fixture).await;
            let no_backend = no_connections(fixture).await;
            (
                success,
                bad_password,
                bad_marker,
                bad_version,
                restored,
                accepted,
                probe,
                no_retry,
                unchanged,
                no_backend,
            )
        })
    })
    .await;
    assert_eq!(success, Ok(()));
    assert_eq!(restored, Ok(()));
    assert_eq!(
        bad_password,
        Err(PlatformHostPreflightError::StorageUnavailable)
    );
    assert_eq!(
        bad_marker,
        Err(PlatformHostPreflightError::StorageMarkersChanged)
    );
    assert!(bad_version.is_err());
    assert!(accepted && no_retry);
    assert_eq!(probe, Err(PlatformHostPreflightError::StorageUnavailable));
    assert!(unchanged && no_backend);
}
