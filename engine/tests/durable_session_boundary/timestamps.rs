use super::*;

const INVALID_EXPIRIES: [&str; 3] = ["infinity", "-infinity", "262143-01-01 00:00:00+00"];

async fn state(f: &Fixture) -> Vec<String> {
    let mut result = Vec::new();
    for table in ["users", "sessions", "collaboration_auth_source_schema"] {
        // PostgreSQL formats even out-of-Chrono timestamps as text. Hash complete synthetic rows
        // inside SQL: snapshotting corruption must not itself decode the offending timestamp.
        result.push(
            query(&format!(
                "SELECT COALESCE(string_agg(md5(row_to_json(t)::text), ','
            ORDER BY md5(row_to_json(t)::text)), '') AS state FROM {table} t"
            ))
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("state"),
        );
    }
    result
}

async fn set_expiry(f: &Fixture, token: &str, timestamp: &str) {
    assert!(INVALID_EXPIRIES.contains(&timestamp));
    query(&format!(
        "UPDATE sessions SET expires_at = '{timestamp}'::timestamptz WHERE token = $1"
    ))
    .bind(token_hash(token))
    .execute(&f.pool)
    .await
    .unwrap();
}

async fn trigger_hits(f: &Fixture) -> i64 {
    query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS calls FROM session_boundary_calls")
        .fetch_one(&f.pool).await.unwrap().get("calls")
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_validate_rejects_unrepresentable_expiry_without_panicking_or_writing(
) {
    for timestamp in INVALID_EXPIRIES {
        let f = Fixture::ready().await;
        let (_, login) = f.seed_account_session().await;
        let valid_before = state(&f).await;
        let valid = f.source.validate_session(&login.token).await;
        let valid_after = state(&f).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
            .bind(token_hash(&login.token)).execute(&f.pool).await.unwrap();
        let expired_before = state(&f).await;
        let expired = f.source.validate_session(&login.token).await;
        let expired_after = state(&f).await;
        set_expiry(&f, &login.token, timestamp).await;
        let corrupt_before = state(&f).await;
        let source = f.source.clone();
        let token = login.token.clone();
        let corrupt = tokio::spawn(async move { source.validate_session(&token).await }).await;
        f.drain().await;
        let corrupt_after = state(&f).await;
        let rows = f.count("sessions").await;
        f.cleanup().await;
        assert_eq!(valid, Ok(Some(login.session)), "{timestamp}");
        assert_eq!(expired, Ok(None), "{timestamp}");
        assert!(
            corrupt.is_ok(),
            "{timestamp}: invalid storage must not panic its caller"
        );
        assert_eq!(
            corrupt.unwrap(),
            Err(DurableAuthError::InvalidRecord),
            "{timestamp}"
        );
        assert_eq!(rows, 1, "reads never clean expired or corrupt records");
        assert_eq!(valid_after, valid_before);
        assert_eq!(expired_after, expired_before);
        assert_eq!(corrupt_after, corrupt_before);
    }

    let f = Fixture::ready().await;
    let (_, login) = f.seed_account_session().await;
    let mut controls = Vec::new();
    for timestamp in [
        "4713-01-01 00:00:00+00 BC",
        "262142-12-31 23:59:59.999999+00",
    ] {
        query(&format!(
            "UPDATE sessions SET expires_at = '{timestamp}'::timestamptz WHERE token = $1"
        ))
        .bind(token_hash(&login.token))
        .execute(&f.pool)
        .await
        .unwrap();
        let before = state(&f).await;
        let source = f.source.clone();
        let token = login.token.clone();
        let result = tokio::spawn(async move { source.validate_session(&token).await }).await;
        f.drain().await;
        let after = state(&f).await;
        controls.push((result, before, after));
    }
    let rows = f.count("sessions").await;
    f.cleanup().await;
    // PostgreSQL timestamps have microsecond precision; Chrono's maximum also has nanoseconds.
    let mut latest = login.session;
    latest.expires_at = chrono::DateTime::<Utc>::from_timestamp_micros(
        chrono::DateTime::<Utc>::MAX_UTC.timestamp_micros(),
    )
    .unwrap();
    for ((result, before, after), expected) in
        controls.into_iter().zip([Ok(None), Ok(Some(latest))])
    {
        assert!(
            result.is_ok(),
            "representable finite boundary must not panic"
        );
        assert_eq!(result.unwrap(), expected);
        assert_eq!(after, before, "finite boundary reads must remain read-only");
    }
    assert_eq!(rows, 1);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_login_rejects_inserted_unrepresentable_expiry_and_rolls_back_otp(
) {
    for timestamp in INVALID_EXPIRIES {
        let f = Fixture::ready().await;
        let created = f.source.register(&username(), PASSWORD).await.unwrap();
        fault(
            &f,
            "BEFORE INSERT",
            &format!("NEW.expires_at = '{timestamp}'::timestamptz; RETURN NEW;"),
        )
        .await;
        let before = state(&f).await;
        let source = f.source.clone();
        let code = otp(&created.bootstrap.secret);
        let result =
            tokio::spawn(
                async move { source.login(&username(), PASSWORD, &code).await.map(|_| ()) },
            )
            .await;
        f.drain().await;
        let after = state(&f).await;
        let reached = trigger_hits(&f).await;
        let sessions = f.count("sessions").await;
        f.cleanup().await;
        assert_eq!(
            reached, 1,
            "{timestamp}: fault must reach the Session write"
        );
        assert!(
            result.is_ok(),
            "{timestamp}: invalid inserted expiry must not panic"
        );
        assert_eq!(
            result.unwrap(),
            Err(DurableAuthError::InvalidRecord),
            "{timestamp}"
        );
        assert_eq!(
            sessions, 0,
            "no token may be published or retained by this rejected transaction"
        );
        assert_eq!(
            after, before,
            "{timestamp}: account, OTP watermark and source roll back together"
        );
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_password_change_rejects_unrepresentable_authorizing_expiry() {
    for timestamp in INVALID_EXPIRIES {
        let f = Fixture::ready().await;
        let (created, login) = f.seed_account_session().await;
        set_expiry(&f, &login.token, timestamp).await;
        let before = state(&f).await;
        let source = f.source.clone();
        let code = otp(&created.bootstrap.secret);
        let result = tokio::spawn(async move {
            source
                .change_session_password(&login.token, PASSWORD, "synthetic-new-password", &code)
                .await
        })
        .await;
        f.drain().await;
        let after = state(&f).await;
        let sessions = f.count("sessions").await;
        f.cleanup().await;
        assert!(
            result.is_ok(),
            "{timestamp}: authorizing Session decode must not panic"
        );
        assert_eq!(
            result.unwrap(),
            Err(DurableAuthError::InvalidRecord),
            "{timestamp}"
        );
        assert_eq!(sessions, 1);
        assert_eq!(
            after, before,
            "{timestamp}: no credentials, OTP watermark or Session changes"
        );
    }
}
