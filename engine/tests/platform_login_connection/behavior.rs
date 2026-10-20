use super::{
    catalog_fixture::Fixture,
    fixture::{
        otp, password, seed_session, session_count, snapshot, username, watermark, Prepared,
    },
    storage_helpers::no_connections,
};
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_exact_issuer_role_and_existing_session_lifecycle() {
    let (issued, validated, denied_logout, logout, revoked, acl, count, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let acl = prepared.minimal_acl().await;
                let connector = prepared.connector().await;
                let origin = connector.origin() == f.config().origin();
                let login = connector.login(&username(), password(), &otp()).await;
                let mut issued = false;
                let mut validated = Ok(false);
                let mut denied_logout = Ok(());
                let mut logout = Err(DurableAuthError::StorageUnavailable);
                let mut revoked = Ok(false);
                if let Ok(login) = login {
                    issued = origin
                        && login.session.account.username == username()
                        && login.session.account.authority_id == f.config().scope().authority_id
                        && login.session.account.account_id.as_uuid().to_string()
                            == "00000000-0000-0000-0000-000000008211"
                        && watermark(f).await >= 0
                        && session_count(f).await == 1;
                    let sessions = prepared.session_connector().await;
                    validated = sessions
                        .validate_session(&login.token)
                        .await
                        .map(|s| s == Some(login.session));
                    // The issuer deliberately has NO DELETE. Existing lifecycle authority is
                    // granted independently by the trusted fixture, never broadened in product.
                    denied_logout = sessions.logout(&login.token).await;
                    f.sql(&format!(
                        "GRANT DELETE ON {}.sessions TO {}",
                        f.source, f.role
                    ))
                    .await;
                    logout = sessions.logout(&login.token).await;
                    revoked = sessions
                        .validate_session(&login.token)
                        .await
                        .map(|s| s.is_none());
                    f.sql(&format!(
                        "REVOKE DELETE ON {}.sessions FROM {}",
                        f.source, f.role
                    ))
                    .await;
                }
                drop(connector);
                let acl = acl && prepared.minimal_acl().await;
                let count = session_count(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    issued,
                    validated,
                    denied_logout,
                    logout,
                    revoked,
                    acl,
                    count,
                    no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(issued && acl);
    assert_eq!(validated, Ok(true));
    assert_eq!(denied_logout, Err(DurableAuthError::StorageUnavailable));
    assert_eq!(logout, Ok(()));
    assert_eq!(revoked, Ok(true));
    assert_eq!(count, 0);
    assert!(no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_wrong_missing_and_rate_limited_attempts_do_not_write() {
    let (failures, missing, wrong_otp, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let connector = prepared.connector().await;
            let before = snapshot(f).await;
            let code = otp();
            let mut failures = Vec::new();
            for _ in 0..6 {
                failures.push(
                    connector
                        .login(&username(), "synthetic-wrong-password", &code)
                        .await
                        .map(|_| ()),
                );
            }
            let missing = connector
                .login(
                    &"synthetic-missing-user".parse().unwrap(),
                    password(),
                    &code,
                )
                .await
                .map(|_| ());
            let independent = prepared.connector().await;
            let wrong_otp = independent
                .login(&username(), password(), "not-six-digits")
                .await
                .map(|_| ());
            drop(independent);
            drop(connector);
            let unchanged = before == snapshot(f).await
                && session_count(f).await == 0
                && watermark(f).await == -1;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (failures, missing, wrong_otp, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert_eq!(
        failures[..5],
        [Err(DurableAuthError::InvalidCredentials); 5]
    );
    assert_eq!(failures[5], Err(DurableAuthError::RateLimited));
    assert_eq!(missing, Err(DurableAuthError::InvalidCredentials));
    assert_eq!(wrong_otp, Err(DurableAuthError::InvalidCredentials));
    assert!(unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_two_connectors_accept_one_otp_once() {
    let (winners, losers, replay, retained, unchanged, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let first = prepared.connector().await;
                let second = prepared.connector().await;
                let code = otp();
                let name = username();
                let (a, b) = tokio::join!(
                    first.login(&name, password(), &code),
                    second.login(&name, password(), &code)
                );
                let winners = usize::from(a.is_ok()) + usize::from(b.is_ok());
                let losers = [&a, &b]
                    .into_iter()
                    .filter(|r| matches!(r, Err(DurableAuthError::InvalidCredentials)))
                    .count();
                let retained = session_count(f).await == 1 && watermark(f).await >= 0;
                let before = snapshot(f).await;
                let replay = first.login(&name, password(), &code).await.map(|_| ());
                drop(first);
                drop(second);
                let unchanged = before == snapshot(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    winners, losers, replay, retained, unchanged, no_backend, cleaned,
                )
            })
        })
        .await;
    assert_eq!(winners, 1);
    assert_eq!(losers, 1);
    assert_eq!(replay, Err(DurableAuthError::InvalidCredentials));
    assert!(retained && unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_full_stock_preserves_otp_for_explicit_retry() {
    let (blocked,unchanged,retried,count,consumed,no_backend,cleaned)=Fixture::checked(|f|Box::pin(async move {
        let prepared=Prepared::new(f).await;
        seed_session(f).await;seed_session(f).await;
        // Stock includes expired rows: login is not an implicit cleanup operation.
        f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 hour'",f.source)).await;
        let connector=prepared.connector().await;
        let code=otp();
        let before=snapshot(f).await;
        let blocked=connector.login(&username(),password(),&code).await.map(|_|());
        let unchanged=before==snapshot(f).await && watermark(f).await== -1 && session_count(f).await==2;
        f.sql(&format!("DELETE FROM {}.sessions WHERE token=(SELECT token FROM {}.sessions ORDER BY token LIMIT 1)",f.source,f.source)).await;
        let retried=connector.login(&username(),password(),&code).await.map(|_|());
        drop(connector);
        let count=session_count(f).await;
        let consumed=watermark(f).await>=0;
        let no_backend=no_connections(f).await;
        let cleaned=prepared.finish();
        (blocked,unchanged,retried,count,consumed,no_backend,cleaned)
    })).await;
    assert_eq!(blocked, Err(DurableAuthError::RateLimited));
    assert_eq!(retried, Ok(()));
    assert_eq!(count, 2);
    assert!(unchanged && consumed && no_backend && cleaned);
}
