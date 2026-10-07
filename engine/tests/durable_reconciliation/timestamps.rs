use super::*;

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_invalid_session_times_return_an_error_without_panicking_or_writing() {
    for expiry in ["infinity", "-infinity", "262143-01-01 00:00:00+00"] {
        let f = CheckFixture::new().await;
        f.owner_login().await;
        query("UPDATE sessions SET expires_at=$1::text::timestamptz")
            .bind(expiry)
            .execute(&f.db.pool)
            .await
            .unwrap();
        let before = f.digest().await;
        let source = f.db.source.clone();
        let result = tokio::spawn(async move { source.reconcile_authority(scope()).await }).await;
        // Observe transaction release and clean even when the old decoder unwinds.
        f.db.drain().await;
        let after = f.digest().await;
        f.cleanup().await;
        assert_eq!(before, after, "read-only reconciliation changed {expiry}");
        let error = result
            .expect("invalid Session time must not panic")
            .unwrap_err();
        assert_eq!(error, ReconciliationError::InvalidSource, "{expiry}");
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_finite_session_time_boundaries_preserve_expiry_counts() {
    let f = CheckFixture::new().await;
    f.owner_login().await;
    for (expiry, expired) in [
        ("4713-01-01 00:00:00+00 BC", 1),
        ("262142-12-31 23:59:59.999999+00", 0),
    ] {
        query("UPDATE sessions SET expires_at=$1::text::timestamptz")
            .bind(expiry)
            .execute(&f.db.pool)
            .await
            .unwrap();
        let before = f.digest().await;
        let report = f.reconcile().await.unwrap();
        assert_eq!(report.sessions, 1);
        assert_eq!(report.expired_sessions, expired, "{expiry}");
        assert_eq!(before, f.digest().await);
    }
    f.cleanup().await;
}
