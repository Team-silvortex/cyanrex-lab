use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    platform_host::{
        PlatformDatabaseTarget, PlatformHostPreflightConfig, PlatformHostPreflightError,
        PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use uuid::Uuid;

pub(super) fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: Uuid::from_u128(4201).try_into().unwrap(),
        workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
    }
}

fn config(origin: &str) -> Result<PlatformHostPreflightConfig, PlatformHostPreflightError> {
    PlatformHostPreflightConfig::new(
        origin,
        scope(),
        PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
        PlatformDatabaseTarget::new("platform_db", "platform_runtime", 123, 456).unwrap(),
        SessionStockLimits::new(4, 16).unwrap(),
    )
}

#[test]
fn platform_host_configuration_preserves_explicit_values_without_opening_storage() {
    let config = config("https://platform.example:8443").unwrap();
    assert_eq!(config.origin(), "https://platform.example:8443");
    assert_eq!(config.scope(), scope());
    assert_eq!(config.namespaces().source(), "source_private");
    assert_eq!(config.namespaces().task(), "task_private");
    assert_eq!(config.namespaces().artifacts(), "artifacts_private");
    assert_eq!(config.target().database(), "platform_db");
    assert_eq!(config.target().role(), "platform_runtime");
    assert_eq!(config.target().system_identifier(), 123);
    assert_eq!(config.target().database_oid(), 456);
    assert_eq!(config.limits().per_account(), 4);
    assert_eq!(config.limits().total(), 16);
    assert!(config == config.clone());
    for (per_account, total) in [(1, 1), (64, 4096)] {
        let limits = SessionStockLimits::new(per_account, total).unwrap();
        let value = PlatformHostPreflightConfig::new(
            "http://127.0.0.1:43123",
            scope(),
            config.namespaces().clone(),
            config.target().clone(),
            limits,
        )
        .unwrap();
        assert_eq!(value.limits(), limits);
    }
}

#[test]
fn platform_host_configuration_requires_a_canonical_https_or_explicit_loopback_origin() {
    for origin in [
        "https://platform.example",
        "https://platform.example:8443",
        "http://localhost:43123",
        "http://127.0.0.1:43123",
        "http://[::1]:43123",
    ] {
        assert!(config(origin).is_ok(), "canonical origin rejected");
    }
    for origin in [
        "",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://platform.example?query=1",
        "https://platform.example#fragment",
        "https://user:password@platform.example",
        "http://192.0.2.1:43123",
        "http://platform.example",
        "file:///tmp/platform",
        "https://*.example",
        " https://platform.example",
        "https://platform.example\n",
    ] {
        assert_eq!(
            config(origin).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidOrigin)
        );
    }
}

#[test]
fn platform_host_namespaces_are_private_bounded_identifiers_and_pairwise_distinct() {
    let longest = format!("a{}", "1".repeat(62));
    assert!(PlatformNamespaces::new(&longest, "task_2", "artifacts_3").is_ok());
    for invalid in [
        "",
        "public",
        "information_schema",
        "pg_catalog",
        "pg_private",
        "9source",
        "Source",
        "source-name",
        "source.name",
        "source name",
        "source;select",
        "source\0",
        "源",
        &"s".repeat(64),
    ] {
        for names in [
            (invalid, "task", "artifacts"),
            ("source", invalid, "artifacts"),
            ("source", "task", invalid),
        ] {
            assert_eq!(
                PlatformNamespaces::new(names.0, names.1, names.2).map(|_| ()),
                Err(PlatformHostPreflightError::InvalidNamespace)
            );
        }
    }
    for names in [
        ("same", "same", "other"),
        ("same", "other", "same"),
        ("other", "same", "same"),
    ] {
        assert_eq!(
            PlatformNamespaces::new(names.0, names.1, names.2).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidNamespace)
        );
    }
}

#[test]
fn platform_host_database_targets_reject_missing_or_malformed_physical_coordinates() {
    let longest = "a".repeat(63);
    let target =
        PlatformDatabaseTarget::new(&longest, "Runtime_1-test", u64::MAX, u32::MAX).unwrap();
    assert_eq!(target.database(), longest);
    assert_eq!(target.role(), "Runtime_1-test");
    assert_eq!(target.system_identifier(), u64::MAX);
    assert_eq!(target.database_oid(), u32::MAX);
    for invalid in [
        "",
        "name space",
        "name.name",
        "name;DROP",
        "name\0",
        "名",
        &"x".repeat(64),
    ] {
        assert_eq!(
            PlatformDatabaseTarget::new(invalid, "runtime", 1, 1).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
        assert_eq!(
            PlatformDatabaseTarget::new("database", invalid, 1, 1).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
    }
    for (system, database) in [(0, 1), (1, 0), (0, 0)] {
        assert_eq!(
            PlatformDatabaseTarget::new("database", "runtime", system, database).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
    }
}
