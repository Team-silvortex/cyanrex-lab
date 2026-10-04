//! Durable Session authority/namespace regressions; no HTTP or live AuthService cutover.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername},
    services::auth_service::durable_source::{DurableAuthError, DurableAuthSource},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "durable_session_boundary/support.rs"]
mod support;
use support::*;
#[path = "durable_session_boundary/concurrency.rs"]
mod concurrency;
#[path = "durable_session_boundary/shapes.rs"]
mod shapes;
#[path = "durable_session_boundary/writes.rs"]
mod writes;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_logout_cannot_confirm_against_redirected_empty_sessions() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    let shadow = format!("session_shadow_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {shadow}"))
        .execute(&f.admin)
        .await
        .unwrap();
    query(&format!(
        "CREATE TABLE {shadow}.sessions (token TEXT PRIMARY KEY)"
    ))
    .execute(&f.admin)
    .await
    .unwrap();
    f.sql(&format!(
        "CREATE FUNCTION redirect_logout() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM set_config('search_path', '{shadow}', true); RETURN NULL; END $$"
    ))
    .await;
    f.sql("CREATE TRIGGER redirect_logout BEFORE DELETE ON sessions FOR EACH ROW EXECUTE FUNCTION redirect_logout()").await;
    let result = f.source.logout(&login.token).await;
    assert_eq!(
        f.count("sessions").await,
        1,
        "the real Session was never revoked"
    );
    assert_eq!(
        f.source.validate_session(&login.token).await.unwrap(),
        Some(login.session)
    );
    query(&format!("DROP SCHEMA {shadow} CASCADE"))
        .execute(&f.admin)
        .await
        .unwrap();
    f.cleanup().await;
    assert!(
        result.is_err(),
        "logout acknowledged an empty shadow relation while the real Session remained valid"
    );
}
