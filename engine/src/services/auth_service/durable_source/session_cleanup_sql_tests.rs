use super::super::{DurableAuthError, DurableAuthSource};
use crate::sqlx_compat::{query, Row};

#[allow(dead_code)]
#[path = "otp_sql_support.rs"]
mod support;
use support::*;

#[path = "session_cleanup_sql_behavior.rs"]
mod behavior;
#[path = "session_cleanup_sql_faults.rs"]
mod faults;

async fn sql(f: &Fixture, statement: &str) {
    query(statement).execute(&f.pool).await.unwrap();
}

async fn fault(f: &Fixture, timing: &str, body: &str) {
    assert!(matches!(timing, "BEFORE DELETE" | "AFTER DELETE"));
    sql(f, "CREATE SEQUENCE cleanup_fault_hits").await;
    sql(
        f,
        &format!(
            "CREATE FUNCTION cleanup_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.cleanup_fault_hits'); {body} END $$",
            f.schema
        ),
    )
    .await;
    sql(f, &format!("CREATE TRIGGER cleanup_fault {timing} ON sessions FOR EACH ROW EXECUTE FUNCTION cleanup_fault()")).await;
}

async fn calls(f: &Fixture) -> i64 {
    query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS calls FROM cleanup_fault_hits")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("calls")
}

async fn drain(f: &Fixture) {
    let mut tx = f.pool.begin().await.unwrap();
    query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await
        .unwrap();
    query("LOCK TABLE users, sessions IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}

async fn insert_sessions(f: &Fixture, start: i32, count: i32, expired: bool) {
    query(
        "INSERT INTO sessions (token, username, account_id, expires_at)
        SELECT lpad(to_hex(n), 64, '0'), username, account_id,
            statement_timestamp() + CASE WHEN $3 THEN INTERVAL '-1 day' ELSE INTERVAL '1 day' END
        FROM users CROSS JOIN generate_series($1, $1 + $2 - 1) n WHERE username = $4",
    )
    .bind(start)
    .bind(count)
    .bind(expired)
    .bind(username().as_str())
    .execute(&f.pool)
    .await
    .unwrap();
}

async fn active_sessions(f: &Fixture) -> Vec<String> {
    query("SELECT token FROM sessions WHERE expires_at > clock_timestamp() ORDER BY token")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("token"))
        .collect()
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_cleanup_prunes_at_most_128_and_preserves_active_sessions_and_accounts() {
    let f = Fixture::ready().await;
    insert_sessions(&f, 1, 130, true).await;
    insert_sessions(&f, 201, 1, false).await;
    // One live row must be an actually usable Session, with a consumed OTP watermark to retain.
    let login = f
        .source
        .login(&username(), PASSWORD, &f.code())
        .await
        .unwrap();
    let session_before = f.source.validate_session(&login.token).await.unwrap();
    let before = f.snapshot().await;
    let active_before = active_sessions(&f).await;
    let first = f.source.prune_expired_sessions().await;
    let after_first = f.count_sessions().await;
    let expired_after_first: Vec<String> =
        query("SELECT token FROM sessions WHERE expires_at <= clock_timestamp() ORDER BY token")
            .fetch_all(&f.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("token"))
            .collect();
    let second = f.source.prune_expired_sessions().await;
    let third = f.source.prune_expired_sessions().await;
    let after = f.snapshot().await;
    let active_after = active_sessions(&f).await;
    let remaining = f.count_sessions().await;
    let session_after = f.source.validate_session(&login.token).await.unwrap();
    f.cleanup().await;
    // The initial no-op implementation reaches complete fixture cleanup before its real Red.
    assert_eq!(first, Ok(128));
    assert_eq!(after_first, 4);
    assert_eq!(
        expired_after_first,
        vec![format!("{:064x}", 129), format!("{:064x}", 130)],
        "equal expiries use the canonical token tie-breaker"
    );
    assert_eq!(second, Ok(2));
    assert_eq!(third, Ok(0));
    assert_eq!(remaining, 2);
    assert_eq!(session_before, Some(login.session));
    assert_eq!(session_after, session_before);
    assert_eq!(active_after, active_before);
    assert_eq!(
        after[0], before[0],
        "accounts, credentials and OTP watermarks stay unchanged"
    );
    assert_eq!(after[2], before[2], "source metadata stays unchanged");
}
