use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*,
    services::file_transfer::{staging::ChunkStagingError, verify_stream},
};
use std::{
    fs::{self, File, OpenOptions, Permissions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    sync::atomic::Ordering,
};

#[tokio::test]
async fn real_cross_chunk_file_stages_private_immutable_chunks_and_verifies_ordered_bytes() {
    let fixture = Fixture::new();
    let expected = manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1);
    let source = fixture.real_file(expected.byte_length());
    let before = snapshot(&source);
    assert!(before.length > 1024 * 1024);
    for index in [0, 1] {
        let spec = expected.chunk(index).unwrap();
        let mut reader = File::open(&source).unwrap();
        reader.seek(SeekFrom::Start(spec.offset)).unwrap();
        fixture
            .store
            .stage_chunk(&expected, index, reader.take(spec.byte_length))
            .await
            .unwrap();
        let staged = snapshot(&fixture.path(&expected, index));
        assert_eq!(staged.length, spec.byte_length);
        assert_eq!(staged.content.as_ref(), Some(&spec.sha256));
        assert_eq!(staged.mode & 0o7777, 0o400);
        assert_eq!(staged.links, 1);
        assert_eq!(staged.owner, fs::metadata(&fixture.root).unwrap().uid());
        let stored = tree(&fixture.root);
        fixture
            .store
            .verify_staged_chunk(&expected, index)
            .await
            .unwrap();
        assert_eq!(tree(&fixture.root), stored);
    }
    let mut ordered = File::open(fixture.path(&expected, 0))
        .unwrap()
        .chain(File::open(fixture.path(&expected, 1)).unwrap());
    verify_stream(&expected, &mut ordered).unwrap();
    assert_eq!(snapshot(&source), before);
    assert_eq!(tree(&fixture.root).len(), 2);
}

#[tokio::test]
async fn full_manifest_and_index_binding_prevents_same_chunk_metadata_aliases() {
    let fixture = Fixture::new();
    let original = manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 2);
    fixture
        .store
        .stage_chunk(&original, 0, Pattern::new(0, FILE_TRANSFER_CHUNK_BYTES))
        .await
        .unwrap();
    let before = tree(&fixture.root);
    assert_eq!(
        fixture.store.verify_staged_chunk(&original, 1).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(tree(&fixture.root), before);
    let variants = [
        changed(
            &original,
            "display_name",
            serde_json::json!("different.bin"),
        ),
        changed(
            &original,
            "file_id",
            serde_json::json!(uuid::Uuid::from_u128(777).to_string()),
        ),
        changed(
            &original,
            "version_id",
            serde_json::json!(uuid::Uuid::from_u128(778).to_string()),
        ),
        changed(
            &original,
            "byte_length",
            serde_json::json!(FILE_TRANSFER_CHUNK_BYTES + 18),
        ),
        changed(
            &original,
            "sha256",
            serde_json::json!(digest(b"another whole-file claim")),
        ),
        changed(
            &original,
            "chunk_sha256",
            serde_json::json!([
                original.chunk_sha256()[0],
                digest(b"another second-chunk claim")
            ]),
        ),
    ];
    for (index, variant) in variants.iter().enumerate() {
        assert_eq!(variant.chunk_sha256()[0], original.chunk_sha256()[0]);
        assert_ne!(name(variant, 0), name(&original, 0));
        let before = tree(&fixture.root);
        assert_eq!(
            fixture.store.verify_staged_chunk(variant, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(tree(&fixture.root), before);
        fixture
            .store
            .stage_chunk(variant, 0, Pattern::new(0, FILE_TRANSFER_CHUNK_BYTES))
            .await
            .unwrap();
        fixture.store.verify_staged_chunk(variant, 0).await.unwrap();
        fixture
            .store
            .verify_staged_chunk(&original, 0)
            .await
            .unwrap();
        assert_eq!(tree(&fixture.root).len(), index + 2);
    }
}

#[tokio::test]
async fn scope_mismatch_and_invalid_indices_do_not_consume_input_or_create_entries() {
    let fixture = Fixture::new();
    let expected = manifest(17, 3);
    let mut foreign_scope = super::scope();
    foreign_scope.workspace_id = uuid::Uuid::from_u128(901).try_into().unwrap();
    let foreign = changed(&expected, "workspace", serde_json::json!(foreign_scope));
    let mut foreign_scope = super::scope();
    foreign_scope.authority_id = uuid::Uuid::from_u128(902).try_into().unwrap();
    let foreign_authority = changed(&expected, "workspace", serde_json::json!(foreign_scope));
    for (expected, index) in [
        (&expected, 1),
        (&expected, u32::MAX),
        (&foreign, 0),
        (&foreign_authority, 0),
    ] {
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert_eq!(
            fixture.store.stage_chunk(expected, index, reader).await,
            Err(ChunkStagingError::InvalidInput)
        );
        assert_eq!(
            fixture.store.verify_staged_chunk(expected, index).await,
            Err(ChunkStagingError::InvalidInput)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(tree(&fixture.root).is_empty());
    }
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
}

#[tokio::test]
async fn corrupt_short_and_long_inputs_retain_unconfirmed_output_never_adopt_or_overwrite() {
    let fixture = Fixture::new();
    for (variant, start, length) in [(10, 1, 17), (11, 0, 16), (12, 0, 1000)] {
        let expected = manifest(17, variant);
        assert_eq!(
            fixture
                .store
                .stage_chunk(&expected, 0, Pattern::new(start, length))
                .await,
            Err(ChunkStagingError::Unconfirmed)
        );
        let retained = snapshot(&fixture.path(&expected, 0));
        assert!(
            retained.length <= 17,
            "overflow sentinel is not output content"
        );
        assert_eq!(
            fixture.store.verify_staged_chunk(&expected, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert_eq!(
            fixture.store.stage_chunk(&expected, 0, reader).await,
            Err(ChunkStagingError::Conflict)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(snapshot(&fixture.path(&expected, 0)), retained);
        let healthy = manifest(17, variant + 100);
        fixture
            .store
            .stage_chunk(&healthy, 0, Pattern::new(0, 17))
            .await
            .unwrap();
        fixture
            .store
            .verify_staged_chunk(&healthy, 0)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn existing_verified_chunk_is_conflict_not_idempotent_adoption() {
    let fixture = Fixture::new();
    let expected = manifest(17, 20);
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert!(tree(&fixture.root).is_empty());
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let before = tree(&fixture.root);
    let reader = Pattern::new(0, 17);
    let calls = reader.calls.clone();
    assert_eq!(
        fixture.store.stage_chunk(&expected, 0, reader).await,
        Err(ChunkStagingError::Conflict)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(tree(&fixture.root), before);
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    assert_eq!(tree(&fixture.root), before);
}

#[tokio::test]
async fn stored_permission_length_and_digest_tampering_fail_with_exact_restore_controls() {
    let fixture = Fixture::new();
    let expected = manifest(17, 21);
    fixture
        .store
        .stage_chunk(&expected, 0, Pattern::new(0, 17))
        .await
        .unwrap();
    let path = fixture.path(&expected, 0);
    for mode in [0o600, 0o440, 0o4000 | 0o400] {
        fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
        let changed = snapshot(&path);
        assert_eq!(
            fixture.store.verify_staged_chunk(&expected, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(snapshot(&path), changed);
        fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
        let restored = snapshot(&path);
        fixture
            .store
            .verify_staged_chunk(&expected, 0)
            .await
            .unwrap();
        assert_eq!(snapshot(&path), restored);
    }
    for length in [16, 18] {
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        let mut output = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut Pattern::new(0, length), &mut output).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
        let changed = snapshot(&path);
        assert_eq!(
            fixture.store.verify_staged_chunk(&expected, 0).await,
            Err(ChunkStagingError::Unavailable)
        );
        assert_eq!(snapshot(&path), changed);
        fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
        output.set_len(0).unwrap();
        output.seek(SeekFrom::Start(0)).unwrap();
        std::io::copy(&mut Pattern::new(0, 17), &mut output).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
        fixture
            .store
            .verify_staged_chunk(&expected, 0)
            .await
            .unwrap();
    }
    fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    let mut output = OpenOptions::new().write(true).open(&path).unwrap();
    output.write_all(&[255]).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
    let changed = snapshot(&path);
    assert_eq!(
        fixture.store.verify_staged_chunk(&expected, 0).await,
        Err(ChunkStagingError::Unavailable)
    );
    assert_eq!(snapshot(&path), changed);
    fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    output.seek(SeekFrom::Start(0)).unwrap();
    output.write_all(&[byte(0)]).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
    let restored = snapshot(&path);
    fixture
        .store
        .verify_staged_chunk(&expected, 0)
        .await
        .unwrap();
    assert_eq!(snapshot(&path), restored);
}
