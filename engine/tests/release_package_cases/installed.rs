use super::{assert_success, set_mode, sha256_file, ArchiveMutation, Fixture};
use axum::{routing::get, Json, Router};
use serde_json::json;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output};
use tokio::net::TcpListener;

fn write_script(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    set_mode(path, 0o755);
}

fn prepare_docker(fixture: &Fixture) {
    fs::create_dir_all(fixture.root.join("bin")).unwrap();
    write_script(
        &fixture.root.join("bin/docker"),
        r#"#!/bin/sh
set -eu
printf 'docker:%s\n' "$*" >> "$TEST_OPERATIONS"
if [ "$*" = "info" ]; then exit 0; fi
[ "$#" -eq 6 ] && [ "$1 $2 $3 $4 $5" = 'image inspect --format {{.Id}} --' ] || exit 2
case "$6" in
  engine:1.2.3) name=engine; digit=e ;;
  frontend:1.2.3) name=frontend; digit=f ;;
  postgres:16) name=postgres; digit=d ;;
  *) exit 2 ;;
esac
if [ "${TEST_DOCKER_MODE:-}" = missing ]; then echo 'No such image' >&2; exit 1; fi
if [ "${TEST_DOCKER_MODE:-}" = invalid ]; then echo 'not-an-image-id'; exit 0; fi
if [ "${TEST_DOCKER_MODE:-}" = oversized ]; then
  while :; do printf '%01024d' 0; done
fi
if [ "${TEST_DOCKER_MODE:-}" = "$name" ]; then digit=0; fi
printf 'sha256:'
i=0
while [ "$i" -lt 64 ]; do printf '%s' "$digit"; i=$((i + 1)); done
printf '\n'
"#,
    );
}

fn test_command(fixture: &Fixture, program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    let mut paths = vec![fixture.root.join("bin")];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    command
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TEST_OPERATIONS", fixture.root.join("operations.log"))
        .env("TEST_PACKAGE", &fixture.package)
        .env("TEST_DOCKER_MODE", "");
    command
}

#[test]
fn installed_image_probe_checks_all_images_and_fails_closed() {
    let fixture = Fixture::new(ArchiveMutation::None);
    prepare_docker(&fixture);
    let probe = |mode: &str| {
        test_command(&fixture, env!("CARGO_BIN_EXE_cyanrex-release"))
            .args(["package", "verify-loaded-images"])
            .arg(&fixture.package)
            .env("TEST_DOCKER_MODE", mode)
            .output()
            .unwrap()
    };
    assert_success(&probe(""));
    let calls = fs::read_to_string(fixture.root.join("operations.log")).unwrap();
    assert_eq!(calls.lines().count(), 3);
    for reference in ["engine:1.2.3", "frontend:1.2.3", "postgres:16"] {
        assert!(calls.contains(&format!(
            "image inspect --format {{{{.Id}}}} -- {reference}"
        )));
    }
    for mode in [
        "engine",
        "frontend",
        "postgres",
        "missing",
        "invalid",
        "oversized",
    ] {
        let output = probe(mode);
        assert!(!output.status.success(), "unexpected success for {mode}");
        let error = String::from_utf8_lossy(&output.stderr);
        let expected = match mode {
            "missing" => "No such image".to_owned(),
            "invalid" => "invalid image ID".to_owned(),
            "oversized" => "output is too large".to_owned(),
            name => format!("loaded {name} image ID"),
        };
        assert!(error.contains(&expected), "wrong error for {mode}: {error}");
    }
    let previous = fs::read(fixture.root.join("operations.log")).unwrap();
    fs::write(fixture.package.join("release-metadata.json"), b"{}\n").unwrap();
    let output = probe("");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("checksum manifest does not match"));
    assert_eq!(
        fs::read(fixture.root.join("operations.log")).unwrap(),
        previous
    );
}

#[test]
fn installed_verifier_rejects_links_missing_files_and_duplicate_metadata() {
    let fixture = Fixture::new(ArchiveMutation::None);
    fs::remove_file(fixture.package.join("run.sh")).unwrap();
    symlink("docker-compose.yml", fixture.package.join("run.sh")).unwrap();
    let output = fixture.verify_installed();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("non-regular member"));

    let missing = Fixture::new(ArchiveMutation::None);
    fs::remove_file(missing.package.join("run.sh")).unwrap();
    let output = missing.verify_installed();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("file set is invalid"));

    let duplicate = Fixture::new(ArchiveMutation::DuplicateMetadataKey);
    let output = duplicate.verify_installed();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate key"));
}

fn installation_fixture() -> Fixture {
    let fixture = Fixture::new(ArchiveMutation::None);
    prepare_docker(&fixture);
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/distribution-install-smoke.sh"),
        fixture.package.join("install-smoke.sh"),
    )
    .unwrap();
    fs::write(
        fixture.package.join("manifest.env"),
        "ENGINE_IMAGE=engine:1.2.3\nFRONTEND_IMAGE=frontend:1.2.3\nPOSTGRES_IMAGE=postgres:16\n",
    )
    .unwrap();
    fs::write(
        fixture.package.join(".env.example"),
        b"# disposable test configuration\n",
    )
    .unwrap();
    for name in [
        "deploy.sh",
        "stop.sh",
        "runner-agent-smoke.sh",
        "live-kernel-smoke.sh",
    ] {
        write_script(
            &fixture.package.join(name),
            &format!("#!/bin/sh\nset -eu\nprintf '{name}:%s\\n' \"$*\" >> \"$TEST_OPERATIONS\"\n"),
        );
    }
    write_script(
        &fixture.package.join("runner-agent.sh"),
        "#!/bin/sh\nprintf 'agent:%s\\n' \"$*\" >> \"$TEST_OPERATIONS\"\n: > \"$TEST_PACKAGE/.runner-agent-token\"\n",
    );
    write_script(
        &fixture.package.join("cyanrex-release"),
        "#!/bin/sh\nprintf 'native:%s\\n' \"$*\" >> \"$TEST_OPERATIONS\"\nexec \"$TEST_RELEASE_CLI\" \"$@\"\n",
    );
    write_script(
        &fixture.root.join("bin/python3"),
        "#!/bin/sh\nprintf 'python-invoked\\n' >> \"$TEST_OPERATIONS\"\nexit 90\n",
    );
    write_script(
        &fixture.root.join("bin/curl"),
        r#"#!/bin/sh
set -eu
[ "$1 $2" = '-fsS -D' ] || exit 2
printf 'Content-Security-Policy: connect-src http://localhost:%s;\r\n' "$CYANREX_SMOKE_ENGINE_PORT" > "$3"
printf '<html>CYANREX</html>\n'
"#,
    );
    let mut manifest = String::new();
    for entry in fs::read_dir(&fixture.package).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name != "checksums.sha256" {
            manifest.push_str(&format!("{}  {name}\n", sha256_file(&entry.path())));
        }
    }
    fs::write(fixture.package.join("checksums.sha256"), manifest).unwrap();
    fixture
}

fn install(fixture: &Fixture, port: u16, mode: &str) -> Output {
    test_command(fixture, "bash")
        .arg(fixture.package.join("install-smoke.sh"))
        .env("TEST_RELEASE_CLI", env!("CARGO_BIN_EXE_cyanrex-release"))
        .env("TEST_DOCKER_MODE", mode)
        .env("CYANREX_SMOKE_KEEP", "0")
        .env("CYANREX_SMOKE_ENGINE_PORT", port.to_string())
        .env("CYANREX_SMOKE_FRONTEND_PORT", "3000")
        .env("CYANREX_SMOKE_BIND_ADDRESS", "127.0.0.1")
        .env("CYANREX_SMOKE_SKIP_AGENT", "0")
        .env("CYANREX_SMOKE_RUN_LIVE_KERNEL", "1")
        .output()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installation_uses_native_probes_without_python_and_cleans_owned_state() {
    let fixture = installation_fixture();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route("/health", get(|| async { Json(json!({"status": "ok"})) }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let output = install(&fixture, port, "");
    server.abort();
    assert_success(&output);
    assert!(!fixture.package.join(".env").exists());
    assert!(!fixture.package.join(".runner-agent-token").exists());
    let calls = fs::read_to_string(fixture.root.join("operations.log")).unwrap();
    for call in [
        "native:package verify ",
        "native:package verify-loaded-images ",
        "native:smoke health --engine-url",
        "deploy.sh:up --pull never",
        "agent:start --agent-id",
        "runner-agent-smoke.sh:",
        "live-kernel-smoke.sh:",
        "deploy.sh:down --volumes --remove-orphans",
    ] {
        assert!(calls.contains(call), "missing {call}: {calls}");
    }
    assert!(!calls.contains("python-invoked"));

    let output = install(&fixture, port, "frontend");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("loaded frontend image ID"));
    assert!(!fixture.package.join(".env").exists());
    let calls = fs::read_to_string(fixture.root.join("operations.log")).unwrap();
    assert!(calls.ends_with("deploy.sh:down --volumes --remove-orphans\n"));
}

#[test]
fn installation_preflight_preserves_preexisting_configuration_and_tokens() {
    let fixture = installation_fixture();
    fs::write(fixture.package.join(".env"), b"existing configuration\n").unwrap();
    fs::write(
        fixture.package.join(".runner-agent-token"),
        b"existing token\n",
    )
    .unwrap();
    let output = install(&fixture, 8080, "");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("refusing to overwrite"));
    assert_eq!(
        fs::read(fixture.package.join(".env")).unwrap(),
        b"existing configuration\n"
    );
    assert_eq!(
        fs::read(fixture.package.join(".runner-agent-token")).unwrap(),
        b"existing token\n"
    );

    fs::remove_file(fixture.package.join(".env")).unwrap();
    let output = install(&fixture, 8080, "");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("existing Runner Agent token"));
    assert_eq!(
        fs::read(fixture.package.join(".runner-agent-token")).unwrap(),
        b"existing token\n"
    );

    symlink("missing-config", fixture.package.join(".env")).unwrap();
    let output = install(&fixture, 8080, "");
    assert!(!output.status.success());
    assert_eq!(
        fs::read_link(fixture.package.join(".env")).unwrap(),
        Path::new("missing-config")
    );
    assert!(!fixture.root.join("operations.log").exists());
}

#[test]
fn installation_checks_the_bundled_binary_before_execution() {
    let fixture = installation_fixture();
    write_script(
        &fixture.package.join("cyanrex-release"),
        "#!/bin/sh\nprintf 'tampered-native\\n' >> \"$TEST_OPERATIONS\"\nexit 0\n",
    );
    let output = install(&fixture, 8080, "");
    assert!(!output.status.success());
    let calls = fs::read_to_string(fixture.root.join("operations.log")).unwrap();
    assert_eq!(calls, "docker:info\n");
    assert!(!fixture.package.join(".env").exists());
}
