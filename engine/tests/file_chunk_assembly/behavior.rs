use crate::{fixture::*, *};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
async fn empty_and_single_chunk_create_real_files_but_bad_empty_root_never_bypasses_checks() {
    for length in [0, 17] {
        let f = Fixture::staged(length).await;
        let before = bytes::tree(&f.root);
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
        let bad = f.staging.assemble_staged_file(&f.manifest).await;
        assert!(matches!(bad, Err(ChunkStagingError::Unavailable)));
        assert_eq!(bytes::tree(&f.root), before);
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
        healthy(&f).await;
        let complete = bytes::tree(&f.root);
        let duplicate = f.staging.assemble_staged_file(&f.manifest).await;
        assert!(matches!(duplicate, Err(ChunkStagingError::Conflict)));
        assert_eq!(bytes::tree(&f.root), complete);
        f.cleanup();
    }
}

#[tokio::test]
async fn scope_is_checked_without_file_access_and_full_manifest_aliases_cannot_adopt_chunks() {
    let f = Fixture::staged(17).await;
    let before = bytes::tree(&f.root);
    let foreign = changed(
        &f.manifest,
        "workspace",
        serde_json::json!({
            "authority_id": scope().authority_id,
            "workspace_id": "00000000-0000-0000-0000-000000000098"
        }),
    );
    // A bad root would produce Unavailable if scope admission dispatched filesystem work.
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
    assert!(matches!(
        f.staging.assemble_staged_file(&foreign).await,
        Err(ChunkStagingError::InvalidInput)
    ));
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
    for (field, value) in [
        ("display_name", serde_json::json!("other.bin")),
        (
            "file_id",
            serde_json::json!("00000000-0000-0000-0000-000000000096"),
        ),
        (
            "version_id",
            serde_json::json!("00000000-0000-0000-0000-000000000097"),
        ),
    ] {
        let alias = changed(&f.manifest, field, value);
        assert_ne!(chunk_name(&alias, 0), chunk_name(&f.manifest, 0));
        assert_ne!(output_name(&alias), output_name(&f.manifest));
        assert!(matches!(
            f.staging.assemble_staged_file(&alias).await,
            Err(ChunkStagingError::Unavailable)
        ));
        assert_eq!(bytes::tree(&f.root), before);
    }
    healthy(&f).await;
    f.cleanup();
}

#[tokio::test]
async fn individually_correct_chunks_with_wrong_whole_digest_fail_before_output_creation() {
    let f = Fixture::staged(FILE_TRANSFER_CHUNK_BYTES + 17).await;
    let wrong = changed(&f.manifest, "sha256", serde_json::json!("a".repeat(64)));
    assert_eq!(wrong.chunk_sha256(), f.manifest.chunk_sha256());
    assert_ne!(wrong.sha256(), f.manifest.sha256());
    f.stage_manifest(&wrong).await;
    for index in [0, 1] {
        f.staging.verify_staged_chunk(&wrong, index).await.unwrap();
    }
    let before = bytes::tree(&f.root);
    let result = f.staging.assemble_staged_file(&wrong).await;
    let after = bytes::tree(&f.root);
    let absent = !f.root.join(output_name(&wrong)).exists();
    healthy(&f).await;
    f.cleanup();
    assert!(matches!(result, Err(ChunkStagingError::Unavailable)));
    assert_eq!(before, after);
    assert!(absent);
}
