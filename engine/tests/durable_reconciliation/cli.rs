#![cfg(unix)]
use super::*;
#[allow(dead_code)]
#[path = "../provision_cli/local.rs"]
mod local;
#[path = "../provision_cli/postgres.rs"]
#[allow(dead_code)]
mod postgres;

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_cli_requires_only_read_privileges_and_never_returns_enrollment() {
    let f = postgres::ProvisionFixture::new().await;
    local::rejected(
        &f.files.run("reconcile"),
        "reconciliation_unsupported_schema",
    );
    assert_eq!(f.db.table_count().await, 0);
    local::success(&f.files.apply(&f.plan()).output().unwrap());
    let enrollment = std::fs::read(&f.files.enrollment).unwrap();
    let token_count = f.db.count("sessions").await;
    let role = format!("reconcile_reader_{}", Uuid::new_v4().simple());
    for statement in [
        format!("CREATE ROLE {role} LOGIN PASSWORD 'synthetic-reconcile-role'"),
        format!("ALTER ROLE {role} SET default_transaction_read_only=on"),
        format!("GRANT USAGE ON SCHEMA {} TO {role}", f.db.schema),
        format!("GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {role}"),
        format!(
            "GRANT SELECT ON ALL TABLES IN SCHEMA {} TO {role}",
            f.db.schema
        ),
        format!("REVOKE SELECT ON users, sessions FROM {role}"),
        format!("GRANT SELECT (username, account_id, otp_last_counter) ON users TO {role}"),
        format!("GRANT SELECT (username, account_id, expires_at, token) ON sessions TO {role}"),
    ] {
        f.db.sql(&statement).await;
    }
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&f.files.config).unwrap()).unwrap();
    config["database_user"] = serde_json::json!(role);
    config["database_password"] = serde_json::json!("synthetic-reconcile-role");
    config["username"] = serde_json::json!("not-an-existing-account");
    f.files.set_config(&config);
    let report = local::success(&f.files.run("reconcile"));
    assert_eq!(report["status"], "consistent_snapshot");
    assert_eq!(report["reconciliation"]["current_managers"], 1);
    assert!(report["target"].get("username").is_none());
    assert!(report.get("confirmation").is_none());
    assert_eq!(std::fs::read(&f.files.enrollment).unwrap(), enrollment);
    assert_eq!(f.db.count("sessions").await, token_count);
    local::rejected(
        &f.files
            .command("reconcile")
            .args(["--confirm", &"a".repeat(64)])
            .output()
            .unwrap(),
        "invalid_arguments",
    );
    f.db.sql("UPDATE collaboration_principals SET display_name='unlogged mutation'")
        .await;
    local::rejected(
        &f.files.run("reconcile"),
        "reconciliation_identity_history_mismatch",
    );
    for statement in [
        format!(
            "REVOKE ALL ON ALL TABLES IN SCHEMA {} FROM {role}",
            f.db.schema
        ),
        format!("REVOKE SELECT (username, account_id, otp_last_counter) ON users FROM {role}"),
        format!("REVOKE SELECT (username, account_id, expires_at, token) ON sessions FROM {role}"),
        format!("REVOKE ALL ON SCHEMA {} FROM {role}", f.db.schema),
        format!("REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {role}"),
        format!("DROP ROLE {role}"),
    ] {
        f.db.sql(&statement).await;
    }
    f.cleanup().await;
}
