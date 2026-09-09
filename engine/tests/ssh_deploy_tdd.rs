use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cyanrex-ssh-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("known_hosts"),
            "# synthetic test fixture; never connect\n",
        )
        .unwrap();
        Self { root }
    }
    fn command(&self, mode: &str) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"));
        cmd.args(["ssh", mode, "--host", "classroom-host", "--directory"])
            .arg(format!(
                "/srv/cyanrex/cyanrex-lab-{}-20260909-010203",
                env!("CARGO_PKG_VERSION")
            ))
            .args(["--action", "up", "--known-hosts"])
            .arg(self.root.join("known_hosts"));
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn ssh_plan_is_offline_strict_and_target_bound() {
    let fixture = Fixture::new();
    let plan = success(fixture.command("plan").output().unwrap());
    let args = plan["ssh_arguments"].as_array().unwrap();
    for option in [
        "StrictHostKeyChecking=yes",
        "BatchMode=yes",
        "ForwardAgent=no",
        "ForwardX11=no",
        "ClearAllForwardings=yes",
        "PermitLocalCommand=no",
        "ControlPath=none",
        "UpdateHostKeys=no",
        "KnownHostsCommand=none",
        "VerifyHostKeyDNS=no",
        "ForkAfterAuthentication=no",
        "SessionType=default",
        "PasswordAuthentication=no",
        "KbdInteractiveAuthentication=no",
    ] {
        assert!(
            args.contains(&Value::String(option.into())),
            "missing {option}"
        );
    }
    assert_eq!(plan["expected_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(plan["changes_remote_state"], true);
    assert!(plan["confirmation"].as_str().unwrap().starts_with("APPLY "));
    assert!(
        String::from_utf8_lossy(&fixture.command("apply").output().unwrap().stderr)
            .contains("--confirm")
    );
    assert!(!fixture
        .command("apply")
        .args(["--confirm", "APPLY wrong-target"])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn ssh_arguments_reject_injection_and_unknown_actions() {
    let fixture = Fixture::new();
    for (option, value) in [
        ("--host", "-oProxyCommand=bad"),
        ("--host", "teacher@host;id"),
        ("--directory", "/srv/$(id)"),
        ("--directory", "/srv/../lab"),
        ("--action", "logs"),
        ("--action", "down --volumes"),
    ] {
        let mut direct = fixture
            .command("plan")
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let index = direct
            .iter()
            .position(|argument| argument == option)
            .unwrap();
        direct[index + 1] = value.into();
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
                .args(direct)
                .output()
                .unwrap()
                .status
                .success(),
            "direct {option} {value}"
        );
        // Duplicate flags are also rejected, never last-value-wins.
        assert!(
            !fixture
                .command("plan")
                .args([option, value])
                .output()
                .unwrap()
                .status
                .success(),
            "{option} {value}"
        );
    }
    let result = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
        .args([
            "ssh",
            "plan",
            "--host",
            "-oProxyCommand=bad",
            "--directory",
            "/srv/cyanrex/cyanrex-lab-0.3.6",
            "--action",
            "up",
            "--known-hosts",
        ])
        .arg(fixture.root.join("known_hosts"))
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[test]
fn ssh_target_refuses_version_mismatch_before_touching_deployment() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
        .args([
            "ssh-target",
            "--expect-version",
            "999.0.0",
            "--action",
            "up",
        ])
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("version mismatch"));
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn ssh_apply_uses_reviewed_arguments_once_and_does_not_retry_failure() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let fake = fixture.root.join("ssh");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$CYANREX_SSH_TEST_OUTPUT\"\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
    let plan = success(fixture.command("plan").output().unwrap());
    let output = fixture
        .command("apply")
        .args(["--confirm", plan["confirmation"].as_str().unwrap()])
        .env("PATH", &fixture.root)
        .env("CYANREX_SSH_TEST_OUTPUT", fixture.root.join("calls"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Inspect status before retrying"));
    let actual = fs::read_to_string(fixture.root.join("calls")).unwrap();
    let expected = plan["ssh_arguments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(
        actual, expected,
        "execute exactly the reviewed argv, without retry"
    );
    fs::write(
        fixture.root.join("known_hosts"),
        "# changed trusted host identity\n",
    )
    .unwrap();
    let rejected = fixture
        .command("apply")
        .args(["--confirm", plan["confirmation"].as_str().unwrap()])
        .env("PATH", &fixture.root)
        .env("CYANREX_SSH_TEST_OUTPUT", fixture.root.join("calls"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(
        fs::read_to_string(fixture.root.join("calls")).unwrap(),
        expected
    );
}

#[cfg(unix)]
#[test]
fn ssh_target_checks_package_controls_and_private_configuration_before_execution() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let version = env!("CARGO_PKG_VERSION");
    let name = format!("cyanrex-lab-{version}-20260909-010203");
    let root = fixture.root.join(&name);
    fs::create_dir(&root).unwrap();
    let deploy = "#!/bin/sh\nprintf '%s' \"$1\" > applied\n";
    for (name, content) in [
        ("deploy.sh", deploy),
        ("docker-compose.yml", "services: {}"),
        ("cyanrex-release", "mock native release binary"),
        ("cyanrex-images.tar", "mock image archive"),
    ] {
        fs::write(root.join(name), content).unwrap();
    }
    fs::set_permissions(root.join("deploy.sh"), fs::Permissions::from_mode(0o700)).unwrap();
    let hash = |name: &str| format!("{:x}", Sha256::digest(fs::read(root.join(name)).unwrap()));
    let metadata = serde_json::json!({
        "schemaVersion": 1,
        "package": {"name":name,"version":version,"createdAt":"2026-09-09T01:02:03Z"},
        "source": {"revision":null,"state":"unavailable","tag":null},
        "compose": {"file":"docker-compose.yml","source":"docker/docker-compose.distribution.yml"},
        "images": {"mode":"prebuilt","references":{"engine":"engine:test","frontend":"frontend:test","postgres":"postgres:16"},
            "contentIds":{"engine":format!("sha256:{}", "a".repeat(64)),"frontend":format!("sha256:{}", "b".repeat(64)),"postgres":format!("sha256:{}", "c".repeat(64))},
            "archive":{"file":"cyanrex-images.tar","sha256":hash("cyanrex-images.tar")}},
        "integrity":{"algorithm":"sha256","manifest":"checksums.sha256"}
    });
    fs::write(root.join("release-metadata.json"), metadata.to_string()).unwrap();
    let manifest = [
        "deploy.sh",
        "docker-compose.yml",
        "cyanrex-release",
        "cyanrex-images.tar",
        "release-metadata.json",
    ]
    .iter()
    .map(|name| format!("{}  {name}\n", hash(name)))
    .collect::<String>();
    fs::write(root.join("checksums.sha256"), manifest).unwrap();
    fs::write(
        root.join(".env"),
        "# synthetic private operator configuration\n",
    )
    .unwrap();
    fs::set_permissions(root.join(".env"), fs::Permissions::from_mode(0o644)).unwrap();
    let target = || {
        Command::new(env!("CARGO_BIN_EXE_cyanrex-release"))
            .args(["ssh-target", "--expect-version", version, "--action", "up"])
            .current_dir(&root)
            .output()
            .unwrap()
    };
    assert!(!target().status.success());
    assert!(!root.join("applied").exists());
    fs::set_permissions(root.join(".env"), fs::Permissions::from_mode(0o600)).unwrap();
    let output = target();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(root.join("applied")).unwrap(), "up");
    fs::remove_file(root.join("applied")).unwrap();
    fs::write(root.join("deploy.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    let rejected = target();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("checksum mismatch"));
    assert!(!root.join("applied").exists());
}
