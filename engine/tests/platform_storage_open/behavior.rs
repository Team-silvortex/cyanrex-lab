use super::fixture::*;
use cyanrex_engine::{
    platform_host::{PlatformLoginConnector, PlatformSessionConnector},
    services::auth_service::durable_source::DurableAuthError,
};
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_uses_declared_binding_for_retained_session_lifecycle() {
    let (
        opened,
        issued,
        validated,
        denied_delete,
        logged_out,
        revoked,
        acl,
        count,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = super::login_fixture::Prepared::new(f).await;
            let catalog = Pins::read(f).await;
            let markers = MarkerPins::read(f).await;
            let root = prepared.root();
            let binding = open(f, root, catalog, markers).await;
            let mut opened = false;
            let mut issued = false;
            let mut validated = false;
            let mut denied_delete = false;
            let mut logged_out = false;
            let mut revoked = false;
            if let Ok(binding) = binding {
                opened = usable(f, &binding).await;
                let login = PlatformLoginConnector::new(binding);
                let result = login
                    .login(
                        &super::login_fixture::username(),
                        super::login_fixture::password(),
                        &super::login_fixture::otp(),
                    )
                    .await;
                if let Ok(result) = result {
                    issued = super::login_fixture::watermark(f).await >= 0
                        && super::login_fixture::session_count(f).await == 1;
                    if let Ok(binding) = open(f, root, catalog, markers).await {
                        let sessions = PlatformSessionConnector::new(binding);
                        validated = sessions
                            .validate_session(&result.token)
                            .await
                            .is_ok_and(|value| value == Some(result.session));
                        denied_delete = sessions.logout(&result.token).await
                            == Err(DurableAuthError::StorageUnavailable);
                        f.sql(&format!(
                            "GRANT DELETE ON {}.sessions TO {}",
                            f.source, f.role
                        ))
                        .await;
                        logged_out = sessions.logout(&result.token).await.is_ok();
                        revoked = sessions
                            .validate_session(&result.token)
                            .await
                            .is_ok_and(|value| value.is_none());
                        f.sql(&format!(
                            "REVOKE DELETE ON {}.sessions FROM {}",
                            f.source, f.role
                        ))
                        .await;
                    }
                }
            }
            let acl = prepared.minimal_acl().await
                && catalog == Pins::read(f).await
                && markers == MarkerPins::read(f).await;
            let count = super::login_fixture::session_count(f).await;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                opened,
                issued,
                validated,
                denied_delete,
                logged_out,
                revoked,
                acl,
                count,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        opened
            && issued
            && validated
            && denied_delete
            && logged_out
            && revoked
            && acl
            && no_backend
            && cleaned
    );
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_readonly_minimal_markers_never_install_or_broaden_permissions(
) {
    let (opened, readonly, forbidden, failures, unchanged, acl, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                prepare(f).await;
                let root = Root::new();
                let catalog = Pins::read(f).await;
                let markers = MarkerPins::read(f).await;
                let before = snapshot(f).await;
                let binding = open(f, &root, catalog, markers).await;
                let mut opened = false;
                let mut readonly = false;
                if let Ok(binding) = binding {
                    opened = usable(f, &binding).await;
                    if let Ok(mut connection) = within(binding.connect()).await {
                        readonly = within(
                            query(
                                "SELECT current_setting('transaction_read_only')='on' AS readonly",
                            )
                            .fetch_one(&mut connection),
                        )
                        .await
                        .unwrap()
                        .get("readonly");
                        within(sqlx_core::connection::Connection::close(connection))
                            .await
                            .unwrap();
                    }
                }
                let forbidden = super::marker_fixture::forbidden_reads_are_denied(f)
                    .await
                    .into_iter()
                    .all(|v| v);
                let mut failures = true;
                let mut unchanged = before == snapshot(f).await;
                // Bad password is not permission to switch credentials, roles, target or a pool fallback.
                failures &= within(
                    cyanrex_engine::platform_host::open_platform_storage_expected(
                        f.config().clone(),
                        endpoint(f),
                        "synthetic-wrong-password",
                        markers.expectation(f, catalog),
                        root.path(),
                        root.original(f.config()),
                    ),
                )
                .await
                .is_err();
                unchanged &= before == snapshot(f).await;
                f.sql(&format!(
                    "REVOKE SELECT ON {} FROM {}",
                    relation(f, 4),
                    f.role
                ))
                .await;
                let changed = snapshot(f).await;
                failures &= open(f, &root, catalog, markers).await.is_err();
                unchanged &= changed == snapshot(f).await;
                f.sql(&format!("GRANT SELECT ON {} TO {}", relation(f, 4), f.role))
                    .await;
                for (index, version) in [(0, 1), (3, 99)] {
                    f.sql(&format!(
                        "UPDATE {} SET version={version}",
                        relation(f, index)
                    ))
                    .await;
                    let changed = snapshot(f).await;
                    failures &= open(f, &root, catalog, markers).await.is_err();
                    unchanged &= changed == snapshot(f).await;
                    f.sql(&format!(
                        "UPDATE {} SET version={}",
                        relation(f, index),
                        if index == 0 { 2 } else { 3 }
                    ))
                    .await;
                }
                f.sql(&format!(
                    "UPDATE {} SET authority_id='00000000-0000-0000-0000-000000009912'",
                    relation(f, 4)
                ))
                .await;
                let changed = snapshot(f).await;
                failures &= open(f, &root, catalog, markers).await.is_err();
                unchanged &= changed == snapshot(f).await;
                f.sql(&format!(
                    "UPDATE {} SET authority_id='{}'",
                    relation(f, 4),
                    f.config().scope().authority_id.as_uuid()
                ))
                .await;
                f.sql(&format!(
                    "ALTER TABLE {} RENAME TO open_saved_absent_marker",
                    relation(f, 4)
                ))
                .await;
                let changed = snapshot(f).await;
                failures &= open(f, &root, catalog, markers).await.is_err();
                unchanged &= changed == snapshot(f).await;
                f.sql(&format!(
                    "ALTER TABLE {}.open_saved_absent_marker RENAME TO {}",
                    f.artifacts, TABLES[4]
                ))
                .await;
                unchanged &= before == snapshot(f).await;
                let acl = super::marker_fixture::only_marker_columns_are_readable(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = root.finish();
                (
                    opened, readonly, forbidden, failures, unchanged, acl, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(
        opened && readonly && forbidden && failures && unchanged && acl && no_backend && cleaned
    );
}
