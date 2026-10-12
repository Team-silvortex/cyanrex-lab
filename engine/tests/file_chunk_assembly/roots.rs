use crate::{fixture::*, *};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
};

#[tokio::test]
async fn changed_ancestor_or_replaced_root_is_not_adopted_and_original_can_be_restored() {
    let f = Fixture::staged(17).await;
    let before = bytes::tree(&f.root);
    fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o770)).unwrap();
    assert!(matches!(
        f.staging.assemble_staged_file(&f.manifest).await,
        Err(ChunkStagingError::Unavailable)
    ));
    assert_eq!(bytes::tree(&f.root), before);
    fs::set_permissions(&f.directory, fs::Permissions::from_mode(0o700)).unwrap();
    let retained = f.directory.join("original-root");
    fs::rename(&f.root, &retained).unwrap();
    fs::DirBuilder::new().mode(0o700).create(&f.root).unwrap();
    // Even copying correct chunks into the replacement root cannot replace the original handle.
    for entry in fs::read_dir(&retained).unwrap() {
        let entry = entry.unwrap();
        let destination = f.root.join(entry.file_name());
        fs::copy(entry.path(), &destination).unwrap();
        fs::set_permissions(destination, fs::Permissions::from_mode(0o400)).unwrap();
    }
    let old_before = bytes::tree(&retained);
    let new_before = bytes::tree(&f.root);
    let result = f.staging.assemble_staged_file(&f.manifest).await;
    assert!(matches!(result, Err(ChunkStagingError::Unavailable)));
    assert_eq!(bytes::tree(&retained), old_before);
    assert_eq!(bytes::tree(&f.root), new_before);
    fs::remove_dir_all(&f.root).unwrap();
    fs::rename(&retained, &f.root).unwrap();
    healthy(&f).await;
    f.cleanup();
}
