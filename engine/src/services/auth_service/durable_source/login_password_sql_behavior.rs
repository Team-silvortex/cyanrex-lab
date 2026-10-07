use super::*;

const PUBLIC_DUMMY_PASSWORD: &str = "cyanrex-public-missing-account-v1";

async fn occupy_executor() -> (BlockingRelease, tokio::task::JoinHandle<bool>) {
    let (release, wait) = std::sync::mpsc::channel();
    let release = BlockingRelease(Some(release));
    let (entered, started) = tokio::sync::oneshot::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _ = entered.send(());
        wait.recv_timeout(Duration::from_secs(10)).is_ok()
    });
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    (release, blocker)
}

async fn dispatched<T>(pending: &tokio::task::JoinHandle<T>, baseline: (usize, usize)) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if pending.is_finished() {
                return false;
            }
            let gate = shared_gate();
            if gate.workers.available_permits() + 1 == baseline.0
                && gate.total.available_permits() + 1 == baseline.1
            {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_login_password_matching_public_dummy_still_cannot_authenticate_or_reach_otp() {
    let f = Fixture::ready().await;
    let before = f.snapshot().await;
    let mut source = f.independent_source(f.pool.clone());
    let reached_otp = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = reached_otp.clone();
    let clock = source.otp_clock.clone().unwrap();
    source.otp_clock = Some(Arc::new(move || {
        observed.store(true, std::sync::atomic::Ordering::SeqCst);
        clock()
    }));
    let missing = "missing-password-learner".parse().unwrap();
    let mut results = Vec::new();
    for password in [PUBLIC_DUMMY_PASSWORD, "wrong-public-password", ""] {
        results.push(
            source
                .login(&missing, password, &f.code())
                .await
                .map(|_| ()),
        );
    }
    let after = f.snapshot().await;
    f.cleanup().await;
    assert!(
        !reached_otp.load(std::sync::atomic::Ordering::SeqCst),
        "a missing snapshot must not reach OTP"
    );
    for result in results {
        assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
    }
    assert_eq!(
        after, before,
        "no missing account, Session or watermark may be created"
    );
}

#[test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
fn postgres_login_password_known_wrong_credentials_take_the_same_bounded_worker_path() {
    isolated_runtime().block_on(async {
        let f = Fixture::ready().await;
        let before = f.snapshot().await;
        let pool = f.separate_pool("known-wrong-password-boundary").await;
        let source = f.independent_source(pool.clone());
        let gate = shared_gate();
        let baseline = (
            gate.workers.available_permits(),
            gate.total.available_permits(),
        );
        let (release, blocker) = occupy_executor().await;
        let pending = tokio::spawn(async move {
            source
                .login(&username(), "incorrect-password", "000000")
                .await
                .map(|_| ())
        });
        let admitted = dispatched(&pending, baseline).await;
        let free = match tokio::time::timeout(Duration::from_millis(500), pool.acquire()).await {
            Ok(Ok(connection)) => {
                drop(connection);
                true
            }
            _ => false,
        };
        let mut probe = f.admin.begin().await.unwrap();
        let unlocked = query(&format!(
            "SELECT version FROM {}.collaboration_auth_source_schema FOR UPDATE NOWAIT",
            f.schema
        ))
        .execute(&mut *probe)
        .await
        .is_ok();
        probe.rollback().await.unwrap();
        drop(release);
        let released = blocker.await.unwrap();
        let result = pending.await.unwrap();
        let after = f.snapshot().await;
        pool.close().await;
        f.cleanup().await;
        assert!(admitted && free && unlocked && released);
        assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
        assert_eq!(after, before);
    });
}

#[test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
fn postgres_login_password_account_created_in_worker_gap_cannot_replace_initial_absence() {
    isolated_runtime().block_on(async {
        let f = Fixture::ready().await;
        // Both the absent snapshot and the later real account must match this public synthetic
        // credential. Otherwise a false dummy result could hide an unsafe second account lookup.
        query("UPDATE users SET password_salt = $1, password_hash = $2 WHERE username = $3")
            .bind("cyanrex-public-missing-account")
            .bind(password_profile::MISSING_ACCOUNT_HASH)
            .bind(username().as_str())
            .execute(&f.pool).await.unwrap();
        let pool = f.separate_pool("missing-created-during-password-work").await;
        let source = f.independent_source(pool.clone());
        let gate = shared_gate();
        let baseline = (gate.workers.available_permits(), gate.total.available_permits());
        let missing: crate::models::collaboration::LegacyUsername = "created-during-password-gap".parse().unwrap();
        let pending_name = missing.clone();
        let code = f.code();
        let submitted = code.clone();
        let (release, blocker) = occupy_executor().await;
        let pending = tokio::spawn(async move {
            source.login(&pending_name, PUBLIC_DUMMY_PASSWORD, &submitted).await.map(|_| ())
        });
        let admitted = dispatched(&pending, baseline).await;
        // Synthetic external registration during the worker gap. Copy supported credentials
        // so that a forbidden second lookup could authenticate, and use a fresh incarnation.
        let inserted = query("INSERT INTO users(username, account_id, password_salt, password_hash, totp_secret)
            SELECT $1, $2, password_salt, password_hash, totp_secret FROM users WHERE username = $3")
            .bind(missing.as_str()).bind(uuid::Uuid::new_v4()).bind(username().as_str())
            .execute(&f.pool).await.unwrap().rows_affected();
        let created_state = f.snapshot().await;
        drop(release);
        let released = blocker.await.unwrap();
        let result = pending.await.unwrap();
        let after = f.snapshot().await;
        let sessions = f.count_sessions().await;
        let fresh_request = f.source.login(&missing, PUBLIC_DUMMY_PASSWORD, &code).await.map(|_| ());
        pool.close().await;
        f.cleanup().await;
        assert!(admitted && released);
        assert_eq!(inserted, 1);
        assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
        assert_eq!(after, created_state, "the original absent snapshot cannot consume the new account's OTP");
        assert_eq!(sessions, 0);
        assert_eq!(fresh_request, Ok(()), "the synthetic concurrent account is valid for a new request");
    });
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_login_password_global_admission_applies_to_existing_and_missing_accounts() {
    let f = Fixture::ready().await;
    let before = f.snapshot().await;
    let gate = shared_gate();
    let total = gate.total.available_permits();
    let held = tokio::time::timeout(
        Duration::from_secs(2),
        gate.total.clone().acquire_many_owned(total as u32),
    )
    .await
    .unwrap()
    .unwrap();
    let known = f
        .source
        .login(&username(), PASSWORD, &f.code())
        .await
        .map(|_| ());
    let missing = f
        .source
        .login(
            &"missing-admission-learner".parse().unwrap(),
            PUBLIC_DUMMY_PASSWORD,
            &f.code(),
        )
        .await
        .map(|_| ());
    drop(held);
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(known, Err(DurableAuthError::RateLimited));
    assert_eq!(missing, Err(DurableAuthError::RateLimited));
    assert_eq!(after, before);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_login_password_input_profile_and_source_errors_are_not_dummy_fallbacks() {
    let f = Fixture::ready().await;
    let before = f.snapshot().await;
    let gate = shared_gate();
    let held = tokio::time::timeout(
        Duration::from_secs(2),
        gate.total
            .clone()
            .acquire_many_owned(gate.total.available_permits() as u32),
    )
    .await
    .unwrap()
    .unwrap();
    let missing = "missing-errors-learner".parse().unwrap();
    let too_long_password = f
        .source
        .login(&missing, &"x".repeat(4097), "000000")
        .await
        .map(|_| ());
    let too_long_otp = f
        .source
        .login(&missing, PASSWORD, &"0".repeat(65))
        .await
        .map(|_| ());
    let maximum_password = f
        .source
        .login(&missing, &"x".repeat(4096), "000000")
        .await
        .map(|_| ());
    let maximum_otp = f
        .source
        .login(&missing, PASSWORD, &"0".repeat(64))
        .await
        .map(|_| ());
    let wrong_source = DurableAuthSource::new(
        f.pool.clone(),
        uuid::Uuid::new_v4().to_string().parse().unwrap(),
    );
    let mismatch = wrong_source
        .login(&missing, PASSWORD, "000000")
        .await
        .map(|_| ());
    let closed_pool = f.separate_pool("missing-login-closed-storage").await;
    closed_pool.close().await;
    let closed = f
        .independent_source(closed_pool)
        .login(&missing, PASSWORD, "000000")
        .await
        .map(|_| ());
    let unmodified = f.snapshot().await;
    query("UPDATE users SET password_hash = '$argon2-invalid' WHERE username = $1")
        .bind(username().as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    let malformed_before = f.snapshot().await;
    let malformed = f
        .source
        .login(&username(), PASSWORD, &f.code())
        .await
        .map(|_| ());
    let malformed_after = f.snapshot().await;
    query("UPDATE collaboration_auth_source_schema SET version = 999")
        .execute(&f.pool)
        .await
        .unwrap();
    let schema_before = f.snapshot().await;
    let unsupported = f
        .source
        .login(&missing, PASSWORD, "000000")
        .await
        .map(|_| ());
    let schema_after = f.snapshot().await;
    let version: i32 = query("SELECT version FROM collaboration_auth_source_schema")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("version");
    drop(held);
    f.cleanup().await;
    assert_eq!(too_long_password, Err(DurableAuthError::InvalidInput));
    assert_eq!(too_long_otp, Err(DurableAuthError::InvalidInput));
    assert_eq!(maximum_password, Err(DurableAuthError::RateLimited));
    assert_eq!(maximum_otp, Err(DurableAuthError::RateLimited));
    assert_eq!(mismatch, Err(DurableAuthError::SourceMismatch));
    assert_eq!(closed, Err(DurableAuthError::StorageUnavailable));
    assert_eq!(malformed, Err(DurableAuthError::InvalidRecord));
    assert_eq!(unsupported, Err(DurableAuthError::UnsupportedSchema));
    assert_eq!(version, 999);
    assert_eq!(unmodified, before);
    assert_eq!(malformed_after, malformed_before);
    assert_eq!(schema_after, schema_before);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_login_password_missing_denials_retain_the_per_username_attempt_budget() {
    let f = Fixture::ready().await;
    let before = f.snapshot().await;
    let missing = "missing-attempt-budget".parse().unwrap();
    let mut results = Vec::new();
    for _ in 0..6 {
        results.push(
            f.source
                .login(&missing, PUBLIC_DUMMY_PASSWORD, "000000")
                .await
                .map(|_| ()),
        );
    }
    let after = f.snapshot().await;
    f.cleanup().await;
    assert_eq!(results[..5], [Err(DurableAuthError::InvalidCredentials); 5]);
    assert_eq!(results[5], Err(DurableAuthError::RateLimited));
    assert_eq!(after, before);
}
