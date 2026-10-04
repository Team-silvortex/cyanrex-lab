use super::*;

pub(super) struct SourceSnapshot {
    pub accounts: HashMap<LegacyAccountId, LegacyUsername>,
    pub sessions: u64,
    pub expired_sessions: u64,
}
pub(super) async fn read(
    connection: &mut PgConnection,
    authority: AuthorityId,
    observed_at: DateTime<Utc>,
) -> CheckResult<SourceSnapshot> {
    budget(
        connection,
        "users",
        "$1::uuid IS NOT NULL",
        "octet_length(username)",
        authority,
    )
    .await?;
    // Only expose whether the fixed-width state is in range, never credentials or OTP material.
    let rows = sqlx::query("SELECT username, account_id, otp_last_counter >= '-1'::bigint AS valid_otp_state FROM users LIMIT 10001")
        .fetch_all(&mut *connection)
        .await?;
    let mut accounts = HashMap::new();
    let mut names = HashSet::new();
    for row in rows {
        let username: LegacyUsername = row
            .try_get::<String, _>("username")?
            .parse()
            .map_err(|_| ReconciliationError::InvalidSource)?;
        let account = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)
            .map_err(|_| ReconciliationError::InvalidSource)?;
        if !row.try_get::<bool, _>("valid_otp_state")?
            || !names.insert(username.clone())
            || accounts.insert(account, username).is_some()
        {
            return Err(ReconciliationError::InvalidSource);
        }
    }
    let sessions = budget(
        connection,
        "sessions",
        "$1::uuid IS NOT NULL",
        "octet_length(username)+octet_length(token)",
        authority,
    )
    .await?;
    // Token digests are validated inside SQL, never selected into the process or report.
    let rows = sqlx::query("SELECT username, account_id, expires_at, token COLLATE \"C\" ~ '^[0-9a-f]{64}$' AS valid_token
        FROM sessions LIMIT 10001").fetch_all(connection).await?;
    let mut expired_sessions = 0;
    for row in rows {
        let name = row
            .try_get::<String, _>("username")?
            .parse::<LegacyUsername>()
            .map_err(|_| ReconciliationError::InvalidSource)?;
        let account = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)
            .map_err(|_| ReconciliationError::InvalidSource)?;
        if !row.try_get::<bool, _>("valid_token")? || accounts.get(&account) != Some(&name) {
            return Err(ReconciliationError::InvalidSource);
        }
        if row.try_get::<DateTime<Utc>, _>("expires_at")? <= observed_at {
            expired_sessions += 1;
        }
    }
    Ok(SourceSnapshot {
        accounts,
        sessions,
        expired_sessions,
    })
}
