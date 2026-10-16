#![cfg(unix)]
//! Public-constructor characterization of the shared policy, without storage or HTTP dispatch.

use cyanrex_engine::{
    models::collaboration::{AuthorityId, WorkspaceRef},
    platform_host::{
        PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformHostPreflightError,
        PlatformNamespaces,
    },
    platform_http::{SessionLifecycleHttpState, TaskContentHttpConfigError},
    services::auth_service::durable_source::{DurableAuthSource, SessionStockLimits},
};
use sqlx_core::connection::ConnectOptions;
use sqlx_postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use uuid::Uuid;

async fn closed_source() -> DurableAuthSource {
    let pool = PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new_without_pgpass()
            .host("127.0.0.1")
            .port(1)
            .username("synthetic_closed_origin_policy")
            .database("synthetic_unopened_origin_policy")
            .password("")
            .ssl_mode(PgSslMode::Disable)
            .disable_statement_logging(),
    );
    pool.close().await;
    assert!(pool.is_closed());
    DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::from_u128(4201)).unwrap())
}

fn host(origin: &str) -> Result<(), PlatformHostPreflightError> {
    let config = PlatformHostPreflightConfig::new(
        origin,
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        },
        PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
        PlatformDatabaseTarget::new("platform_db", "platform_runtime", 42, 43).unwrap(),
        SessionStockLimits::new(4, 16).unwrap(),
    )?;
    assert_eq!(
        config.origin(),
        origin,
        "the constructor must not normalize input"
    );
    Ok(())
}

fn check(source: &DurableAuthSource, origin: &str, accepted: bool) {
    let host = host(origin);
    let http = SessionLifecycleHttpState::new(source.clone(), origin).map(|_| ());
    if accepted {
        assert_eq!(host, Ok(()), "host rejected {origin:?}");
        assert_eq!(http, Ok(()), "HTTP rejected {origin:?}");
    } else {
        assert_eq!(
            host,
            Err(PlatformHostPreflightError::InvalidOrigin),
            "host changed rejection for {origin:?}"
        );
        assert_eq!(
            http,
            Err(TaskContentHttpConfigError::InvalidOrigin),
            "HTTP changed rejection for {origin:?}"
        );
    }
}

#[tokio::test]
async fn platform_origin_public_constructors_accept_canonical_https_and_explicit_http_loopback() {
    let source = closed_source().await;
    for origin in [
        "https://platform.example",
        "https://platform.example:8443",
        "https://platform.example:80",
        "https://platform.example:0",
        "https://platform.example:65535",
        "https://platform.example.",
        "https://localhost",
        "https://localhost.",
        "https://127.0.0.1",
        "https://192.168.1.20:8443",
        "https://0.0.0.0",
        "https://[::1]",
        "https://[::]",
        "https://[2001:db8::1]:8443",
        "https://[::ffff:7f00:1]",
        "https://xn--fiqs8s.example",
        "http://localhost",
        "http://localhost:43123",
        "http://localhost:443",
        "http://localhost:0",
        "http://localhost:65535",
        "http://127.0.0.0",
        "http://127.0.0.1",
        "http://127.1.2.3:43123",
        "http://127.255.255.255:43123",
        "http://[::1]",
        "http://[::1]:43123",
        "http://[::1]:0",
    ] {
        check(&source, origin, true);
    }
}

#[tokio::test]
async fn platform_origin_public_constructors_reject_noncanonical_or_unsafe_origin_forms() {
    let source = closed_source().await;
    for origin in [
        "",
        "platform.example",
        "//platform.example",
        "https://",
        "HTTPS://platform.example",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example:08443",
        "https://platform.example:65536",
        "https://platform.example/path",
        "https://platform.example/.",
        "https://platform.example?",
        "https://platform.example?query=1",
        "https://platform.example#",
        "https://platform.example#fragment",
        "https://user@platform.example",
        "https://:password@platform.example",
        "https://user:password@platform.example",
        "https://@platform.example",
        "https://*.example",
        "https://foo*.example",
        "https://%2A.example",
        "https://平台.example",
        "https://127.1",
        "https://[0:0:0:0:0:0:0:1]",
        " https://platform.example",
        "https://platform.example ",
        "https://platform.example\n",
        "https://plat\tform.example",
        "https://platform.example\0",
        "http://platform.example",
        "http://192.168.1.20:43123",
        "http://0.0.0.0",
        "http://126.255.255.255",
        "http://128.0.0.1",
        "http://127.0.0.1.example",
        "http://localhost.example",
        "http://localhost.",
        "http://LOCALHOST",
        "http://localhost:80",
        "http://127.1",
        "http://2130706433",
        "http://0x7f000001",
        "http://[::]",
        "http://[::2]",
        "http://[::ffff:7f00:1]",
        "http://[fe80::1%25eth0]",
        "ws://localhost",
        "wss://platform.example",
        "ftp://platform.example",
        "file:///tmp/platform",
        "null",
    ] {
        check(&source, origin, false);
    }
}

#[tokio::test]
async fn platform_origin_public_constructors_preserve_the_2048_byte_limit() {
    let source = closed_source().await;
    for length in [1024, 2047, 2048, 2049, 4096] {
        // URL parsing historically allows these long ASCII hosts; the policy's separate byte
        // cap is not an added DNS-label, resolver or complete hostname validity guarantee.
        let origin = format!("https://{}.example", "a".repeat(length - 16));
        assert_eq!(origin.len(), length);
        check(&source, &origin, length <= 2048);
    }
}
