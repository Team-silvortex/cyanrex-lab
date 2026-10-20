use super::fixture::{config, endpoint, environment, expected, listener, no_dial, Root, PASSWORD};
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformStorageConnectionError,
};
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const CASE: &str = "CYANREX_STORAGE_OPEN_DEFAULT_CASE";
const DIRECTORY: &str = "CYANREX_STORAGE_OPEN_DEFAULT_DIRECTORY";
const COMPLETE: &str = "synthetic-storage-open-native-case-complete";

struct Pending(Option<Child>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(super) fn run_case(case: &str) -> bool {
    let before = environment();
    let container = Root::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "environment::platform_storage_open_environment_child",
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .env(CASE, case)
        .env(DIRECTORY, container.path())
        .env("PGHOST", "untrusted.invalid")
        .env("PGHOSTADDR", "192.0.2.91")
        .env("PGPORT", "65432")
        .env("PGUSER", "untrusted_user")
        .env("PGDATABASE", "untrusted_database")
        .env("PGPASSWORD", "public-synthetic-ambient-password")
        .env("PGAPPNAME", "untrusted_application")
        .env("PGSSLMODE", "verify-full")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Only the child's explicitly controlled environment changes. The parent never clears PG*.
    for name in [
        "PGOPTIONS",
        "PGPASSFILE",
        "PGSSLROOTCERT",
        "PGSSLCERT",
        "PGSSLKEY",
    ] {
        command.env_remove(name);
    }
    if case == "options-empty" {
        command.env("PGOPTIONS", "");
    }
    if case == "options-value" {
        command.env("PGOPTIONS", "-c role=untrusted_role");
    }
    let mut child = Pending(Some(command.spawn().unwrap()));
    let deadline = Instant::now() + Duration::from_secs(18);
    let finished = loop {
        if child.0.as_mut().unwrap().try_wait().unwrap().is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let success = if finished {
        let output = child.0.take().unwrap().wait_with_output().unwrap();
        output.status.success()
            && output.stderr.is_empty()
            && String::from_utf8_lossy(&output.stdout)
                .matches(COMPLETE)
                .count()
                == 1
            && !String::from_utf8_lossy(&output.stdout).contains(PASSWORD)
    } else {
        false
    };
    drop(child);
    let unchanged = before == environment();
    let cleaned = container.finish();
    success && unchanged && cleaned
}

#[test]
fn platform_storage_open_defaults_password_and_pgoptions_reject_only_in_isolated_children() {
    let outcomes = [
        "password-long",
        "password-nul",
        "options-empty",
        "options-value",
    ]
    .map(run_case);
    assert!(outcomes.into_iter().all(|passed| passed));
}

#[test]
#[ignore = "explicit subprocess helper, exercised by every named native environment/peer test"]
fn platform_storage_open_environment_child() {
    let case = std::env::var(CASE).expect("explicit synthetic child case required");
    let base = std::path::PathBuf::from(
        std::env::var_os(DIRECTORY).expect("owned child container required"),
    );
    assert!(base.is_absolute());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        if case.starts_with("peer-") {
            super::peer::exercise(&case, &base).await;
            return;
        }
        let root = Root::new_in(&base);
        let listener = listener().await;
        let password = match case.as_str() {
            "password-long" => "x".repeat(4097),
            "password-nul" => "synthetic\0password".to_owned(),
            "options-empty" | "options-value" => PASSWORD.to_owned(),
            _ => panic!("unknown synthetic child case"),
        };
        let result = open_platform_storage_expected(
            config(0),
            endpoint(&listener),
            &password,
            expected(config(0)),
            root.path(),
            root.original(&config(0)),
        )
        .await
        .map(|_| ());
        let no_dial = no_dial(&listener).await;
        drop(listener);
        let cleaned = root.finish();
        assert_eq!(result, Err(PlatformStorageConnectionError::InvalidConfig));
        assert!(no_dial && cleaned);
    });
    drop(runtime);
    println!("{COMPLETE}");
}
