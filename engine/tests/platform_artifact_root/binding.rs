use super::fixture::{directory, Fixture};
use cyanrex_engine::{
    platform_host::{
        inspect_platform_artifact_root, verify_platform_artifact_root, PlatformDatabaseTarget,
        PlatformHostPreflightConfig, PlatformHostPreflightError as Error, PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};
use uuid::Uuid;

fn changed_configs(config: &PlatformHostPreflightConfig) -> Vec<PlatformHostPreflightConfig> {
    let make = |origin, scope, namespaces, target, limits| {
        PlatformHostPreflightConfig::new(origin, scope, namespaces, target, limits).unwrap()
    };
    let mut changed = Vec::new();
    changed.push(make(
        "https://other.example",
        config.scope(),
        config.namespaces().clone(),
        config.target().clone(),
        config.limits(),
    ));
    let mut scope = config.scope();
    scope.authority_id = Uuid::from_u128(6403).try_into().unwrap();
    changed.push(make(
        config.origin(),
        scope,
        config.namespaces().clone(),
        config.target().clone(),
        config.limits(),
    ));
    let mut scope = config.scope();
    scope.workspace_id = Uuid::from_u128(6404).try_into().unwrap();
    changed.push(make(
        config.origin(),
        scope,
        config.namespaces().clone(),
        config.target().clone(),
        config.limits(),
    ));
    for namespaces in [
        PlatformNamespaces::new(
            "other_source",
            config.namespaces().task(),
            config.namespaces().artifacts(),
        )
        .unwrap(),
        PlatformNamespaces::new(
            config.namespaces().source(),
            "other_task",
            config.namespaces().artifacts(),
        )
        .unwrap(),
        PlatformNamespaces::new(
            config.namespaces().source(),
            config.namespaces().task(),
            "other_artifacts",
        )
        .unwrap(),
    ] {
        changed.push(make(
            config.origin(),
            config.scope(),
            namespaces,
            config.target().clone(),
            config.limits(),
        ));
    }
    let target = config.target();
    for target in [
        PlatformDatabaseTarget::new(
            "other_database",
            target.role(),
            target.system_identifier(),
            target.database_oid(),
        )
        .unwrap(),
        PlatformDatabaseTarget::new(
            target.database(),
            "other_role",
            target.system_identifier(),
            target.database_oid(),
        )
        .unwrap(),
        PlatformDatabaseTarget::new(
            target.database(),
            target.role(),
            target.system_identifier() + 1,
            target.database_oid(),
        )
        .unwrap(),
        PlatformDatabaseTarget::new(
            target.database(),
            target.role(),
            target.system_identifier(),
            target.database_oid() + 1,
        )
        .unwrap(),
    ] {
        changed.push(make(
            config.origin(),
            config.scope(),
            config.namespaces().clone(),
            target,
            config.limits(),
        ));
    }
    for limits in [
        SessionStockLimits::new(1, 8).unwrap(),
        SessionStockLimits::new(2, 9).unwrap(),
    ] {
        changed.push(make(
            config.origin(),
            config.scope(),
            config.namespaces().clone(),
            config.target().clone(),
            limits,
        ));
    }
    changed
}

#[test]
fn platform_artifact_root_binds_every_config_field_and_exact_os_path_spelling() {
    let (configs, aliases, non_utf8, unchanged, original_healthy) = Fixture::checked(|fixture| {
        let original = inspect_platform_artifact_root(&fixture.config, &fixture.root)
            .unwrap_or_else(|_| panic!("original root observation unavailable"));
        let mut configs = Vec::new();
        for config in changed_configs(&fixture.config) {
            let verified = verify_platform_artifact_root(&config, &fixture.root, &original);
            let fresh = inspect_platform_artifact_root(&config, &fixture.root);
            let different = fresh
                .as_ref()
                .is_ok_and(|fresh| !original.matches(fresh) && !fresh.matches(&original));
            configs.push((verified, fresh.map(|_| ()), different));
        }
        let mut trailing = fixture.root.as_os_str().to_os_string();
        trailing.push("/");
        let mut repeated = fixture.directory().as_os_str().to_os_string();
        repeated.push("//private-artifacts");
        let paths = [
            PathBuf::from(trailing),
            fixture.root.join("."),
            PathBuf::from(repeated),
        ];
        let mut aliases = Vec::new();
        let before = fixture.snapshot();
        for path in paths {
            let actually_different = path.as_os_str() != fixture.root.as_os_str();
            let verified = verify_platform_artifact_root(&fixture.config, &path, &original);
            let fresh = inspect_platform_artifact_root(&fixture.config, &path);
            let (different, own_verify) = match &fresh {
                Ok(fresh) => (
                    !original.matches(fresh),
                    verify_platform_artifact_root(&fixture.config, &path, fresh),
                ),
                Err(error) => (false, Err(*error)),
            };
            aliases.push((
                verified,
                fresh.map(|_| ()),
                different,
                own_verify,
                actually_different,
            ));
        }
        let unchanged = before == fixture.snapshot();
        let non_utf8_path = fixture
            .directory()
            .join(OsString::from_vec(vec![b'r', b'o', b'o', b't', 0xff]));
        directory(&non_utf8_path);
        let non_utf8 = inspect_platform_artifact_root(&fixture.config, &non_utf8_path).map(|_| ());
        let original_healthy =
            verify_platform_artifact_root(&fixture.config, &fixture.root, &original);
        drop(original);
        (configs, aliases, non_utf8, unchanged, original_healthy)
    });
    assert_eq!(configs.len(), 12);
    for (verified, fresh, different) in configs {
        assert_eq!(verified, Err(Error::ArtifactRootChanged));
        assert_eq!(fresh, Ok(()));
        assert!(different);
    }
    assert_eq!(aliases.len(), 3);
    for (verified, fresh, different, own_verify, actually_different) in aliases {
        assert_eq!(verified, Err(Error::ArtifactRootChanged));
        assert_eq!(fresh, Ok(()));
        assert_eq!(own_verify, Ok(()));
        assert!(different && actually_different);
    }
    assert_eq!(non_utf8, Ok(()));
    assert!(unchanged);
    assert_eq!(original_healthy, Ok(()));
}
