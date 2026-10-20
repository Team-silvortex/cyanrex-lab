use super::fixture::{config, endpoint, expected, listener, no_dial, Root, PASSWORD};
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformStorageConnectionError,
};

#[tokio::test]
async fn platform_storage_open_defaults_complete_configuration_mismatch_rejects_without_dial() {
    let root = Root::new();
    let listener = listener().await;
    let mut results = Vec::new();
    for change in 1..=12 {
        results.push(
            open_platform_storage_expected(
                config(change),
                endpoint(&listener),
                PASSWORD,
                expected(config(0)),
                root.path(),
                root.original(&config(change)),
            )
            .await
            .map(|_| ()),
        );
    }
    let no_dial = no_dial(&listener).await;
    drop(listener);
    let cleaned = root.finish();
    assert_eq!(
        results,
        vec![Err(PlatformStorageConnectionError::InvalidConfig); 12]
    );
    assert!(no_dial && cleaned);
}

#[tokio::test]
async fn platform_storage_open_defaults_root_binding_and_raw_spelling_mismatch_never_dial() {
    let root = Root::new();
    let listener = listener().await;
    let paths = [
        root.path().join("."),
        root.path().with_file_name("unobserved-artifacts"),
        std::path::PathBuf::from(format!("{}/", root.path().display())),
    ];
    let mut results = Vec::new();
    for path in paths {
        results.push(
            open_platform_storage_expected(
                config(0),
                endpoint(&listener),
                PASSWORD,
                expected(config(0)),
                &path,
                root.original(&config(0)),
            )
            .await
            .map(|_| ()),
        );
    }
    results.push(
        open_platform_storage_expected(
            config(0),
            endpoint(&listener),
            PASSWORD,
            expected(config(0)),
            root.path(),
            root.original(&config(1)),
        )
        .await
        .map(|_| ()),
    );
    let no_dial = no_dial(&listener).await;
    drop(listener);
    let cleaned = root.finish();
    assert_eq!(
        results,
        vec![Err(PlatformStorageConnectionError::InvalidConfig); 4]
    );
    assert!(no_dial && cleaned);
}

#[tokio::test]
async fn platform_storage_open_defaults_replaced_or_unsafe_original_root_is_not_repaired_or_dialed()
{
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
    };
    let root = Root::new();
    let listener = listener().await;
    let original = root.original(&config(0));
    root.swap();
    let replacement = open_platform_storage_expected(
        config(0),
        endpoint(&listener),
        PASSWORD,
        expected(config(0)),
        root.path(),
        original,
    )
    .await
    .map(|_| ());
    let mut no_attempt = no_dial(&listener).await;
    root.restore();
    let original = root.original(&config(0));
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o750)).unwrap();
    let unsafe_root = open_platform_storage_expected(
        config(0),
        endpoint(&listener),
        PASSWORD,
        expected(config(0)),
        root.path(),
        original,
    )
    .await
    .map(|_| ());
    let not_repaired = fs::symlink_metadata(root.path()).unwrap().mode() & 0o7777 == 0o750;
    no_attempt &= no_dial(&listener).await;
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    drop(listener);
    let cleaned = root.finish();
    assert_eq!(
        replacement,
        Err(PlatformStorageConnectionError::Unavailable)
    );
    assert_eq!(
        unsafe_root,
        Err(PlatformStorageConnectionError::Unavailable)
    );
    assert!(no_attempt && not_repaired && cleaned);
}
