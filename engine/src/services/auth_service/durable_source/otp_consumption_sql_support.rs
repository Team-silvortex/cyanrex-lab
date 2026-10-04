use super::*;

pub(super) async fn sql(f: &Fixture, statement: &str) {
    query(statement).execute(&f.pool).await.unwrap();
}

pub(super) async fn counter(f: &Fixture, name: &str) -> i64 {
    query("SELECT otp_last_counter FROM users WHERE username = $1")
        .bind(name)
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("otp_last_counter")
}

pub(super) fn clock_counter(f: &Fixture) -> i64 {
    f.clock
        .load(std::sync::atomic::Ordering::SeqCst)
        .div_euclid(30)
}

pub(super) async fn setup_rotation(f: &Fixture) -> String {
    let login = f
        .source
        .login(&username(), PASSWORD, &f.code())
        .await
        .unwrap();
    f.next_code();
    login.token
}

pub(super) async fn command(
    source: &DurableAuthSource,
    token: Option<&str>,
    code: &str,
) -> Result<(), DurableAuthError> {
    if let Some(token) = token {
        source
            .change_session_password(token, PASSWORD, NEW_PASSWORD, code)
            .await
    } else {
        source.login(&username(), PASSWORD, code).await.map(|_| ())
    }
}

pub(super) async fn fault(f: &Fixture, table: &str, timing: &str, body: &str) {
    assert!(matches!(table, "users" | "sessions"));
    sql(f, "CREATE SEQUENCE consumption_fault_hits").await;
    sql(
        f,
        &format!(
            "CREATE FUNCTION consumption_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.consumption_fault_hits'); {body} END $$",
            f.schema
        ),
    )
    .await;
    sql(f, &format!("CREATE TRIGGER consumption_fault {timing} ON {table} FOR EACH ROW EXECUTE FUNCTION consumption_fault()" )).await;
}

pub(super) async fn calls(f: &Fixture) -> i64 {
    query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS calls FROM consumption_fault_hits")
        .fetch_one(&f.pool).await.unwrap().get("calls")
}

pub(super) async fn drain(f: &Fixture) {
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
