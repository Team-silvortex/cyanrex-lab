use super::{options, PlatformDatabaseConnectionError, PlatformDatabaseEndpoint};
use crate::{
    models::collaboration::WorkspaceRef,
    platform_host::{PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformNamespaces},
    services::auth_service::durable_source::SessionStockLimits,
};
use sqlx_core::connection::ConnectOptions;
use sqlx_postgres::{PgConnectOptions, PgSslMode};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsString},
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::unix::{ffi::OsStringExt, fs::DirBuilderExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

fn config() -> PlatformHostPreflightConfig {
    PlatformHostPreflightConfig::new(
        "https://platform.example:8443",
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        },
        PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
        PlatformDatabaseTarget::new("platform_db", "platform_runtime", 123, 456).unwrap(),
        SessionStockLimits::new(4, 16).unwrap(),
    )
    .unwrap()
}

#[test]
fn platform_connection_unix_endpoint_requires_a_canonical_absolute_utf8_directory() {
    for path in [
        "",
        ".",
        "relative",
        "/",
        "/tmp/",
        "/tmp//socket",
        "//tmp/socket",
        "/tmp/./socket",
        "/tmp/../socket",
        "/tmp/socket/.",
        "/tmp/socket/..",
        "/tmp/socket\0hidden",
        "/tmp/socket\n",
        "/tmp/socket\r",
        "/tmp/socket\t",
        "/tmp/socket\u{7f}",
        "/tmp/socket\u{85}",
    ] {
        assert!(matches!(
            PlatformDatabaseEndpoint::unix(Path::new(path), 5432),
            Err(PlatformDatabaseConnectionError::InvalidConfig)
        ));
    }
    let invalid_utf8 = PathBuf::from(OsString::from_vec(b"/tmp/\xff".to_vec()));
    assert!(matches!(
        PlatformDatabaseEndpoint::unix(&invalid_utf8, 5432),
        Err(PlatformDatabaseConnectionError::InvalidConfig)
    ));
    for path in ["/tmp/platform", "/tmp/目录"] {
        assert!(PlatformDatabaseEndpoint::unix(Path::new(path), 5432).is_ok());
    }
    assert!(matches!(
        PlatformDatabaseEndpoint::unix(Path::new("/tmp/platform"), 0),
        Err(PlatformDatabaseConnectionError::InvalidConfig)
    ));
}

#[test]
fn platform_connection_unix_endpoint_budgets_the_complete_socket_name_in_bytes() {
    // '/' + 84 bytes + '/.s.PGSQL.65535' is exactly 100 bytes.
    let longest = format!("/{}", "s".repeat(84));
    assert_eq!(format!("{longest}/.s.PGSQL.65535").len(), 100);
    assert!(PlatformDatabaseEndpoint::unix(Path::new(&longest), 65535).is_ok());
    assert!(matches!(
        PlatformDatabaseEndpoint::unix(Path::new(&format!("{longest}s")), 65535),
        Err(PlatformDatabaseConnectionError::InvalidConfig)
    ));
    let multibyte = format!("/{}", "界".repeat(28));
    assert_eq!(multibyte.len(), longest.len());
    assert!(PlatformDatabaseEndpoint::unix(Path::new(&multibyte), 65535).is_ok());
    assert!(matches!(
        PlatformDatabaseEndpoint::unix(Path::new(&format!("{multibyte}界")), 65535),
        Err(PlatformDatabaseConnectionError::InvalidConfig)
    ));
}

#[test]
fn platform_connection_loopback_endpoint_accepts_only_literal_loopback_and_nonzero_ports() {
    for address in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
        IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255)),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        for port in [1, 5432, u16::MAX] {
            assert!(PlatformDatabaseEndpoint::loopback(address, port).is_ok());
        }
        assert!(matches!(
            PlatformDatabaseEndpoint::loopback(address, 0),
            Err(PlatformDatabaseConnectionError::InvalidConfig)
        ));
    }
    for address in [
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V4(Ipv4Addr::new(126, 255, 255, 255)),
        IpAddr::V4(Ipv4Addr::new(128, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
        IpAddr::V4(Ipv4Addr::BROADCAST),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        "::ffff:127.0.0.1".parse().unwrap(),
        "fe80::1".parse().unwrap(),
    ] {
        assert!(matches!(
            PlatformDatabaseEndpoint::loopback(address, 5432),
            Err(PlatformDatabaseConnectionError::InvalidConfig)
        ));
    }
}

fn pg_environment() -> BTreeMap<OsString, OsString> {
    std::env::vars_os()
        .filter(|(name, _)| name.as_encoded_bytes().starts_with(b"PG"))
        .collect()
}

fn run_child(case: &str, passfile: Option<&Path>) -> bool {
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let mut command = Command::new(executable);
    command
        .args([
            "platform_host::database_connection::tests::environment_child",
            "--ignored",
            "--exact",
        ])
        .env("CYANREX_CONNECTOR_ENV_CASE", case)
        .env("PGHOST", "untrusted.invalid")
        .env("PGHOSTADDR", "192.0.2.91")
        .env("PGPORT", "65432")
        .env("PGUSER", "ambient_user")
        .env("PGDATABASE", "ambient_database")
        .env("PGPASSWORD", "synthetic_ambient_password")
        .env("PGAPPNAME", "ambient_application")
        .env("PGSSLMODE", "verify-full")
        .env("PGSSLROOTCERT", "/nonexistent/ambient_root_certificate")
        .env("PGSSLCERT", "/nonexistent/ambient_client_certificate")
        .env("PGSSLKEY", "/nonexistent/ambient_private_key")
        .env_remove("PGOPTIONS")
        .env_remove("PGPASSFILE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match case {
        "startup-options" => {
            command.env("PGOPTIONS", "-c role=ambient_role");
        }
        "empty-startup-options" => {
            command.env("PGOPTIONS", "");
        }
        "passfile" => {
            command
                .env_remove("PGPASSWORD")
                .env("PGPASSFILE", passfile.unwrap());
        }
        _ => {}
    }
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                // This is only the directly spawned child, never another process or process group.
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[test]
fn platform_connection_options_ignore_ambient_values_and_preserve_parent_environment() {
    let before = pg_environment();
    let success = run_child("override", None);
    let unchanged = before == pg_environment();
    assert!(success, "explicit connection option child failed");
    assert!(unchanged, "parent PG environment changed");
}

#[test]
fn platform_connection_options_reject_ambient_startup_options_even_when_empty() {
    let before = pg_environment();
    let nonempty = run_child("startup-options", None);
    let empty = run_child("empty-startup-options", None);
    let unchanged = before == pg_environment();
    assert!(
        nonempty && empty,
        "ambient startup options were not rejected"
    );
    assert!(unchanged, "parent PG environment changed");
}

#[test]
fn platform_connection_password_limits_are_explicit_and_byte_bounded() {
    let before = pg_environment();
    let success = run_child("passwords", None);
    let unchanged = before == pg_environment();
    assert!(success, "explicit password bounds child failed");
    assert!(unchanged, "parent PG environment changed");
}

#[test]
fn platform_connection_options_never_open_an_ambient_passfile_fifo() {
    let before = pg_environment();
    let directory =
        PathBuf::from("/tmp").join(format!("cyanrex-connector-options-{}", Uuid::new_v4()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let fifo = directory.join("passfile");
    let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: this is a NUL-free filename inside this test's newly created private directory.
    let created = unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0;
    let success = created && run_child("passfile", Some(&fifo));
    let removed_fifo = !created || fs::remove_file(&fifo).is_ok();
    let removed_directory = fs::remove_dir(&directory).is_ok();
    let unchanged = before == pg_environment();
    assert!(
        removed_fifo && removed_directory,
        "owned FIFO cleanup failed"
    );
    assert!(
        created && success,
        "ambient passfile was read or options were rejected"
    );
    assert!(unchanged, "parent PG environment changed");
}

fn assert_coordinates(actual: &PgConnectOptions, host: &str, socket: Option<&Path>, port: u16) {
    assert!(actual.get_host() == host, "explicit host changed");
    assert!(
        actual.get_socket().map(PathBuf::as_path) == socket,
        "explicit transport changed"
    );
    assert_eq!(actual.get_port(), port);
    assert!(
        actual.get_username() == "platform_runtime",
        "explicit role changed"
    );
    assert!(
        actual.get_database() == Some("platform_db"),
        "explicit database changed"
    );
    assert!(matches!(actual.get_ssl_mode(), PgSslMode::Disable));
    assert!(
        actual.get_application_name() == Some("cyanrex-platform-connection"),
        "explicit application name changed"
    );
    assert!(
        actual.get_options() == Some("-c search_path=source_private -c statement_timeout=5000 -c lock_timeout=2000 -c idle_in_transaction_session_timeout=10000"),
        "startup GUCs must be source-only and bounded"
    );
}

fn assert_password_and_cache(actual: &PgConnectOptions, password: &str) {
    // Synthetic test credentials only. Never format the options or URL into failure output.
    let url = actual.to_url_lossy();
    let exact_password =
        sqlx_core::percent_encoding::percent_decode_str(url.password().unwrap_or_default())
            .decode_utf8()
            .is_ok_and(|decoded| decoded == password);
    assert!(exact_password, "explicit password changed");
    assert!(
        url.query_pairs()
            .any(|(key, value)| { key == "statement-cache-capacity" && value == "100" }),
        "statement cache capacity changed"
    );
}

#[test]
#[ignore = "only the bounded parent subprocess tests may supply the explicit child mode"]
fn environment_child() {
    let mode = std::env::var("CYANREX_CONNECTOR_ENV_CASE")
        .expect("environment helper requires an explicit parent-owned child mode");
    let environment = pg_environment();
    let config = config();
    let socket = Path::new("/tmp/cyanrex-explicit-platform");
    let unix = PlatformDatabaseEndpoint::unix(socket, 55478).unwrap();
    match mode.as_str() {
        "override" | "passfile" => {
            let actual = options(&config, &unix, "synthetic_explicit_password")
                .expect("explicit connection options rejected");
            assert_coordinates(&actual, "localhost", Some(socket), 55478);
            assert_password_and_cache(&actual, "synthetic_explicit_password");
            let ipv4 =
                PlatformDatabaseEndpoint::loopback(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 55479)
                    .unwrap();
            let actual = options(&config, &ipv4, "synthetic_explicit_password").unwrap();
            assert_coordinates(&actual, "127.0.0.2", None, 55479);
            assert_password_and_cache(&actual, "synthetic_explicit_password");
            let ipv6 =
                PlatformDatabaseEndpoint::loopback(IpAddr::V6(Ipv6Addr::LOCALHOST), 55480).unwrap();
            let actual = options(&config, &ipv6, "synthetic_explicit_password").unwrap();
            assert_coordinates(&actual, "::1", None, 55480);
        }
        "startup-options" | "empty-startup-options" => {
            assert!(matches!(
                options(&config, &unix, "synthetic_explicit_password"),
                Err(PlatformDatabaseConnectionError::AmbientStartupOptions)
            ));
        }
        "passwords" => {
            for password in [String::new(), "x".repeat(4096), "界".repeat(1365)] {
                let actual =
                    options(&config, &unix, &password).expect("valid explicit password rejected");
                assert_coordinates(&actual, "localhost", Some(socket), 55478);
                assert_password_and_cache(&actual, &password);
            }
            for password in [
                "x".repeat(4097),
                "界".repeat(1366),
                "before\0after".to_owned(),
            ] {
                assert!(matches!(
                    options(&config, &unix, &password),
                    Err(PlatformDatabaseConnectionError::InvalidConfig)
                ));
            }
        }
        _ => panic!("unknown explicit environment child mode"),
    }
    assert!(
        environment == pg_environment(),
        "connection options mutated child environment"
    );
}
