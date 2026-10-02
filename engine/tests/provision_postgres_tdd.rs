#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername},
    services::auth_service::durable_source::DurableAuthSource,
};
use serde_json::json;
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
};
use uuid::Uuid;
#[allow(dead_code)]
#[path = "provision_cli/local.rs"]
mod local;
#[path = "provision_cli/postgres.rs"]
mod postgres;
#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use local::{rejected, success, PASSWORD};
use postgres::ProvisionFixture;

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_plan_and_inspect_are_read_only_and_ignore_ambient_configuration() {
    let f = ProvisionFixture::new().await;
    let first = f.plan();
    assert_eq!(first, f.plan());
    let report = success(&f.files.run("inspect"));
    assert_eq!(report["namespace_state"], "empty");
    assert!(report.get("confirmation").is_none());
    assert!(report.get("healthy").is_none());
    assert_eq!(f.db.table_count().await, 0);
    assert!(!f.files.enrollment.exists());
    assert_eq!(fs::read_dir(&f.files.root).unwrap().count(), 2);
    // Let PostgreSQL itself enforce read-only operation. This non-superuser has the explicit
    // schema/control-read privileges, but its role defaults prohibit DDL/DML even during apply.
    let role = format!("provision_readonly_{}", Uuid::new_v4().simple());
    for statement in [
        format!("CREATE ROLE {role} LOGIN PASSWORD 'synthetic-read-only-role'"),
        format!("ALTER ROLE {role} SET default_transaction_read_only = on"),
        format!("GRANT USAGE, CREATE ON SCHEMA {} TO {role}", f.db.schema),
        format!("GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {role}"),
    ] {
        f.db.sql(&statement).await;
    }
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(&f.files.config).unwrap()).unwrap();
    config["database_user"] = json!(role);
    config["database_password"] = json!("synthetic-read-only-role");
    f.files.set_config(&config);
    let confirmation = f.plan();
    assert_eq!(success(&f.files.run("inspect"))["namespace_state"], "empty");
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "bootstrap_unconfirmed",
    );
    assert_eq!(f.db.table_count().await, 0);
    for statement in [
        format!("REVOKE ALL ON SCHEMA {} FROM {role}", f.db.schema),
        format!("REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM {role}"),
        format!("DROP ROLE {role}"),
    ] {
        f.db.sql(&statement).await;
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_apply_delivers_private_enrollment_only_after_atomic_commit() {
    let f = ProvisionFixture::new().await;
    f.files
        .write(&f.files.password, format!("{PASSWORD}\r\n").as_bytes());
    let output = f.files.apply(&f.plan()).output().unwrap();
    let report = success(&output);
    assert_eq!(report["status"], "committed_enrollment_saved");
    assert_eq!(
        fs::metadata(&f.files.enrollment)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let enrollment = f.enrollment();
    assert_eq!(enrollment["status"], "committed");
    assert_eq!(enrollment["account_id"], report["account_id"]);
    assert_eq!(enrollment["principal_id"], report["principal_id"]);
    assert_eq!(enrollment["confirmation"], report["confirmation"]);
    let secret = enrollment["totp"]["secret"].as_str().unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    assert_eq!(f.db.count("users").await, 1);
    assert_eq!(f.db.count("sessions").await, 0);
    let login =
        f.db.source
            .login(
                &"first-teacher".parse().unwrap(),
                PASSWORD,
                &source_fixture::otp(secret),
            )
            .await
            .unwrap();
    assert_eq!(
        login.session.account.account_id.to_string(),
        report["account_id"]
    );
    let inspect = success(&f.files.run("inspect"));
    assert_eq!(inspect["namespace_state"], "occupied");
    assert!(inspect.get("healthy").is_none());
    assert!(inspect.get("confirmation").is_none());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_confirmation_binds_target_intent_and_namespace_incarnation() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    rejected(
        &f.files.apply(&"a".repeat(64)).output().unwrap(),
        "confirmation_mismatch",
    );
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(&f.files.config).unwrap()).unwrap();
    for (key, value) in [
        ("username", json!("other-teacher")),
        ("authority_id", json!(Uuid::new_v4())),
        ("workspace_id", json!(Uuid::new_v4())),
    ] {
        let mut changed = original.clone();
        changed[key] = value;
        f.files.set_config(&changed);
        rejected(
            &f.files.apply(&confirmation).output().unwrap(),
            "confirmation_mismatch",
        );
    }
    f.files.set_config(&original);
    f.db.sql(&format!("DROP SCHEMA {}", f.db.schema)).await;
    query(&format!("CREATE SCHEMA {}", f.db.schema))
        .execute(&f.db.admin)
        .await
        .unwrap();
    assert_ne!(confirmation, f.plan());
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "confirmation_mismatch",
    );
    assert_eq!(f.db.table_count().await, 0);
    assert!(!f.files.enrollment.exists());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_existing_or_partial_namespaces_are_never_adopted_or_replayed() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    f.db.source.install_empty_schema().await.unwrap();
    rejected(&f.files.run("plan"), "namespace_not_empty");
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "namespace_not_empty",
    );
    assert_eq!(f.db.count("users").await, 0);
    assert!(!f.files.enrollment.exists());
    f.cleanup().await;
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    success(&f.files.apply(&confirmation).output().unwrap());
    let original = fs::read(&f.files.enrollment).unwrap();
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "namespace_not_empty",
    );
    assert_eq!(fs::read(&f.files.enrollment).unwrap(), original);
    assert_eq!(f.db.count("users").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_unsafe_or_existing_secret_destinations_prevent_database_writes() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    f.files
        .write(&f.files.enrollment, b"preserve-existing-file");
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "enrollment_unavailable",
    );
    assert_eq!(
        fs::read(&f.files.enrollment).unwrap(),
        b"preserve-existing-file"
    );
    fs::remove_file(&f.files.enrollment).unwrap();
    symlink(&f.files.password, &f.files.enrollment).unwrap();
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "enrollment_unavailable",
    );
    assert_eq!(fs::read(&f.files.password).unwrap(), PASSWORD.as_bytes());
    fs::remove_file(&f.files.enrollment).unwrap();
    fs::set_permissions(&f.files.root, fs::Permissions::from_mode(0o755)).unwrap();
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "enrollment_unavailable",
    );
    fs::set_permissions(&f.files.root, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(f.db.table_count().await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_competing_processes_publish_only_one_bootstrap_secret() {
    let mut f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    let first_path = f.files.enrollment.clone();
    let first = f.spawn(&confirmation);
    f.files.enrollment = f.files.root.join("other-enrollment.json");
    let second = f.spawn(&confirmation);
    let results = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    assert_eq!(results.iter().filter(|r| r.status.success()).count(), 1);
    let mut committed = 0;
    for path in [&first_path, &f.files.enrollment] {
        if let Ok(bytes) = fs::read(path) {
            let content: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if content["status"] == "committed" {
                committed += 1;
            } else {
                assert!(content.get("totp").is_none());
            }
        }
    }
    assert_eq!(committed, 1);
    for result in &results {
        local::redacted(result);
    }
    assert_eq!(f.db.count("users").await, 1);
    assert_eq!(f.db.count("sessions").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_unconfirmed_write_retains_marker_and_never_automatically_retries() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    let fence = f.fence().await;
    let child = f.spawn(&confirmation);
    f.blocked().await;
    assert_eq!(f.enrollment()["status"], "pending_reconciliation");
    assert!(f.enrollment().get("totp").is_none());
    rejected(&child.wait_with_output().unwrap(), "bootstrap_unconfirmed");
    fence.rollback().await.unwrap();
    assert_eq!(f.db.table_count().await, 0);
    assert_eq!(success(&f.files.run("inspect"))["namespace_state"], "empty");
    rejected(
        &f.files.apply(&confirmation).output().unwrap(),
        "enrollment_unavailable",
    );
    assert_eq!(f.db.table_count().await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_post_commit_delivery_failure_preserves_database_and_replaced_file() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    let fence = f.fence().await;
    let child = f.spawn(&confirmation);
    f.blocked().await;
    fs::remove_file(&f.files.enrollment).unwrap();
    f.files
        .write(&f.files.enrollment, b"do-not-overwrite-replacement");
    fence.rollback().await.unwrap();
    rejected(
        &child.wait_with_output().unwrap(),
        "committed_delivery_unconfirmed",
    );
    assert_eq!(f.db.count("users").await, 1);
    assert_eq!(f.db.count("sessions").await, 0);
    assert_eq!(
        fs::read(&f.files.enrollment).unwrap(),
        b"do-not-overwrite-replacement"
    );
    rejected(&f.files.run("plan"), "namespace_not_empty");
    f.cleanup().await;

    // Exercise a real short write after COMMIT, not just a preflight/path rejection.
    use std::os::unix::process::CommandExt;
    let f = ProvisionFixture::new().await;
    let mut command = f.files.apply(&f.plan());
    // SAFETY: only async-signal-safe libc operations occur in the forked test child.
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            let limit = libc::rlimit {
                rlim_cur: 512,
                rlim_max: 512,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    rejected(&command.output().unwrap(), "committed_delivery_unconfirmed");
    assert_eq!(f.db.count("users").await, 1);
    assert_eq!(f.db.count("sessions").await, 0);
    let partial = fs::read(&f.files.enrollment).unwrap();
    assert_eq!(partial.len(), 512);
    assert!(serde_json::from_slice::<serde_json::Value>(&partial).is_err());
    assert_eq!(
        fs::metadata(&f.files.enrollment)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    rejected(&f.files.run("plan"), "namespace_not_empty");
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_provision_process_cancellation_never_removes_evidence_or_seeds_a_session() {
    let f = ProvisionFixture::new().await;
    let confirmation = f.plan();
    let fence = f.fence().await;
    let mut child = f.spawn(&confirmation);
    f.blocked().await;
    child.kill().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    fence.rollback().await.unwrap();
    assert_eq!(f.enrollment()["status"], "pending_reconciliation");
    assert!(f.enrollment().get("totp").is_none());
    assert_eq!(f.db.table_count().await, 0);
    f.cleanup().await;
}
