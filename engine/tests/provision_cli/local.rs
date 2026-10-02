use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
};
use uuid::Uuid;

pub const PASSWORD: &str = "synthetic-provision-password";
pub struct Local {
    pub root: PathBuf,
    pub config: PathBuf,
    pub password: PathBuf,
    pub enrollment: PathBuf,
}
impl Local {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cyanrex-provision-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let result = Self {
            config: root.join("target.json"),
            password: root.join("password"),
            enrollment: root.join("enrollment.json"),
            root,
        };
        result.write(&result.password, PASSWORD.as_bytes());
        result.set_config(&Self::target());
        result
    }
    pub fn target() -> Value {
        json!({"format_version":1,
            "transport":{"kind":"unix", "directory":"/no-such-cyanrex-socket"},
            "port":5432, "database":"cyanrex_test", "database_user":"cyanrex_test",
            "database_password":"synthetic-database-secret", "schema":"fresh_authority",
            "authority_id":"1223c6e9-823d-4cea-998a-a92f019b35fc",
            "workspace_id":"60a9845d-0687-4f19-a4f5-2ebf64bed4ba", "username":"first-teacher"})
    }
    pub fn write(&self, path: &Path, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
    }
    pub fn set_config(&self, config: &Value) {
        self.write(&self.config, &serde_json::to_vec(config).unwrap());
    }
    pub fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-provision"));
        command
            .arg(verb)
            .arg("--config")
            .arg(&self.config)
            .env("PGHOST", "untrusted.invalid")
            .env("PGPORT", "1")
            .env("PGHOSTADDR", "untrusted.invalid")
            .env("PGUSER", "wrong-user")
            .env("PGDATABASE", "wrong-database")
            .env("PGPASSWORD", "do-not-print-this-environment-secret")
            .env(
                "PGOPTIONS",
                "-c search_path=public -c default_transaction_read_only=on",
            )
            .env("PGSSLMODE", "verify-full")
            .env("PGPASSFILE", self.root.join("not-a-passfile"))
            .env(
                "DATABASE_URL",
                "postgres://never-connect:environment-secret@untrusted.invalid/db",
            )
            .env("CYANREX_ADMIN_PASSWORD", "never-use-live-auth-seed");
        command
    }
    pub fn run(&self, verb: &str) -> Output {
        self.command(verb).output().unwrap()
    }
    pub fn apply(&self, confirmation: &str) -> Command {
        let mut command = self.command("apply");
        command
            .arg("--confirm")
            .arg(confirmation)
            .arg("--password-file")
            .arg(&self.password)
            .arg("--enrollment-file")
            .arg(&self.enrollment);
        command
    }
}
impl Drop for Local {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let result = serde_json::from_slice(&output.stdout).unwrap();
    redacted(output);
    result
}
pub fn rejected(output: &Output, code: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let result: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(result["error"], code);
    redacted(output);
}
pub fn redacted(output: &Output) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for forbidden in [
        PASSWORD,
        "synthetic-database-secret",
        "environment-secret",
        "never-use-live-auth-seed",
        "otpauth://",
        "password_hash",
        "totp_secret",
    ] {
        assert!(
            !text.contains(forbidden),
            "secret or credential field appeared in console output"
        );
    }
    if let Ok(password) = std::env::var("CYANREX_TEST_DATABASE_PASSWORD") {
        if !password.is_empty() {
            assert!(
                !text.contains(&password),
                "disposable DB password appeared in console output"
            );
        }
    }
}
