use super::*;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

#[tokio::test]
async fn absent_store_reads_default_without_creating_directories() {
    let fixture = Fixture::new();
    let result = fixture.store().get().await.unwrap();
    assert_eq!(result.revision, 0);
    assert!(result.profiles.is_empty());
    assert!(!fixture.0.exists());
}

#[tokio::test]
async fn cloned_stores_compare_revision_and_keep_private_atomic_files() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let clone = store.clone();
    let (one, two) = tokio::join!(store.update(update(0)), clone.update(update(0)));
    assert_eq!(
        [one.is_ok(), two.is_ok()]
            .into_iter()
            .filter(|ok| *ok)
            .count(),
        1
    );
    assert!(
        one == Err(AiAgentSettingsError::Conflict) || two == Err(AiAgentSettingsError::Conflict)
    );
    assert_eq!(store.get().await.unwrap().revision, 1);
    assert_eq!(std::fs::metadata(&fixture.0).unwrap().mode() & 0o777, 0o700);
    let path = fixture.0.join("settings.json");
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    let before = std::fs::read(&path).unwrap();
    let mut overflow: serde_json::Value = serde_json::from_slice(&before).unwrap();
    overflow["revision"] = u32::MAX.into();
    std::fs::write(&path, serde_json::to_vec(&overflow).unwrap()).unwrap();
    assert_eq!(
        store.update(update(u32::MAX)).await,
        Err(AiAgentSettingsError::InvalidInput)
    );
    assert_eq!(store.get().await.unwrap().revision, u32::MAX);
}

#[tokio::test]
async fn corrupt_oversize_or_unsafe_files_never_get_overwritten() {
    let fixture = Fixture::new();
    let store = fixture.store();
    store.update(update(0)).await.unwrap();
    let path = fixture.0.join("settings.json");
    let valid = std::fs::read(&path).unwrap();
    for bytes in [
        b"not-json".to_vec(),
        vec![b' '; 32769],
        b"{}".to_vec(),
        b"[1,null,[]]".to_vec(),
    ] {
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            store.get().await,
            Err(AiAgentSettingsError::StorageUnavailable)
        );
        assert_eq!(
            store.update(update(1)).await,
            Err(AiAgentSettingsError::StorageUnavailable)
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::write(&path, &valid).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        store.get().await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(
        store.update(update(1)).await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(std::fs::read(&path).unwrap(), valid);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        store.get().await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(
        store.update(update(1)).await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(std::fs::metadata(&fixture.0).unwrap().mode() & 0o777, 0o755);
}

#[tokio::test]
async fn symlink_file_and_directory_targets_are_not_followed() {
    let fixture = Fixture::new();
    let store = fixture.store();
    store.update(update(0)).await.unwrap();
    let path = fixture.0.join("settings.json");
    let other = fixture.0.join("untouched.json");
    std::fs::rename(&path, &other).unwrap();
    symlink(&other, &path).unwrap();
    let before = std::fs::read(&other).unwrap();
    assert_eq!(
        store.get().await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(
        store.update(update(1)).await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(std::fs::read(&other).unwrap(), before);
    let alias = fixture.0.join("alias");
    symlink(&fixture.0, &alias).unwrap();
    assert_eq!(
        AiAgentSettingsStore::new(alias).get().await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
}
