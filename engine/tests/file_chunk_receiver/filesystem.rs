use crate::{local::*, *};
use cyanrex_engine::services::file_transfer::staging::ChunkStagingError;
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, DirBuilderExt, PermissionsExt},
    },
};

#[tokio::test]
async fn receiver_never_adopts_overwrites_or_deletes_any_existing_destination() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        for mode in 0..6 {
            let manifest = bytes::manifest(17, mode + 1);
            let path = f.root.join(fixture::legacy_name(&manifest, 0));
            let anchor = f.directory.join(format!("anchor-{mode}"));
            fs::write(&anchor, data(&manifest, 0)).unwrap();
            match mode {
                0 => {
                    fs::write(&path, data(&manifest, 0)).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
                }
                1 => {
                    fs::write(&path, b"wrong").unwrap();
                }
                2 => {
                    symlink(&anchor, &path).unwrap();
                }
                3 => {
                    fs::hard_link(&anchor, &path).unwrap();
                }
                4 => {
                    fs::create_dir(&path).unwrap();
                }
                _ => {
                    let p = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(p.as_ptr(), 0o600) }, 0);
                }
            }
            let before = f.tree();
            let anchor_before = bytes::snapshot(&anchor);
            assert_eq!(
                receiver
                    .receive_and_stage(
                        &manifest,
                        0,
                        response(&manifest, 0, Body::from(data(&manifest, 0)))
                    )
                    .await
                    .err(),
                Some(FileChunkReceiveError::Staging(ChunkStagingError::Conflict))
            );
            assert_eq!(f.tree(), before);
            assert_eq!(bytes::snapshot(&anchor), anchor_before);
            if mode == 4 {
                fs::remove_dir(&path).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
            healthy(&f, &receiver, mode + 100).await;
        }
    })
    .await;
}

#[tokio::test]
async fn receiver_private_root_mode_and_incarnation_drift_refuse_and_recover_without_repairs() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
        let injected = bytes::snapshot(&f.root);
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            response(&manifest, 0, Body::from(data(&manifest, 0))),
            FileChunkReceiveError::Staging(ChunkStagingError::Unavailable),
        )
        .await;
        assert_eq!(bytes::snapshot(&f.root), injected);
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
        healthy(&f, &receiver, 2).await;
        let saved = f.directory.join("original-root");
        fs::rename(&f.root, &saved).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&f.root).unwrap();
        let original = bytes::tree(&saved);
        let old_root = bytes::snapshot(&saved);
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            response(&manifest, 0, Body::from(data(&manifest, 0))),
            FileChunkReceiveError::Staging(ChunkStagingError::Unavailable),
        )
        .await;
        assert_eq!(bytes::tree(&saved), original);
        assert_eq!(bytes::snapshot(&saved), old_root);
        fs::remove_dir(&f.root).unwrap();
        fs::rename(&saved, &f.root).unwrap();
        healthy(&f, &receiver, 3).await;
    })
    .await;
}

#[tokio::test]
async fn receiver_complete_validation_precedes_staging_and_full_manifest_binding_is_exact() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        healthy(&f, &receiver, 1).await;
        // The selected bytes and file/version stay unchanged; only an inert manifest label differs.
        let mut raw = serde_json::to_value(&manifest).unwrap();
        raw["display_name"] = serde_json::json!("another-label.bin");
        let distinct: FileTransferManifest = serde_json::from_value(raw).unwrap();
        receiver
            .receive_and_stage(
                &distinct,
                0,
                response(&distinct, 0, Body::from(data(&distinct, 0))),
            )
            .await
            .unwrap();
        assert_ne!(
            fixture::legacy_name(&manifest, 0),
            fixture::legacy_name(&distinct, 0)
        );
        assert_eq!(f.tree().len(), 2);
        for m in [&manifest, &distinct] {
            fixture::observe_chunk(&f.root.join(fixture::legacy_name(m, 0)), m, 0)
                .unwrap()
                .verified
                .unwrap();
        }
        // A body fault wins before even consulting the already occupied destination.
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            response(
                &manifest,
                0,
                crate::script::Script::new([crate::script::Step::Error]),
            ),
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
    })
    .await;
}

#[tokio::test]
async fn receiver_selected_tail_can_be_staged_without_other_chunks_or_fresh_whole_hash_proof() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1);
        // A syntactically valid whole hash is a manifest binding, not a recomputed chunk hash.
        let mut raw = serde_json::to_value(&manifest).unwrap();
        raw["sha256"] = serde_json::json!("3".repeat(64));
        let claimed: FileTransferManifest = serde_json::from_value(raw).unwrap();
        let received = receiver
            .receive_and_stage(
                &claimed,
                1,
                response(&claimed, 1, Body::from(data(&claimed, 1))),
            )
            .await
            .unwrap();
        assert_eq!(received.file(), &FileVersionRef::from_manifest(&claimed));
        assert_eq!(received.chunk_index(), 1);
        assert_eq!(received.byte_length(), 17);
        let entries = f.tree();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].name,
            std::ffi::OsString::from(fixture::legacy_name(&claimed, 1))
        );
        let observed = fixture::observe_chunk(&f.root.join(&entries[0].name), &claimed, 1).unwrap();
        observed.verified.unwrap();
        assert_eq!(observed.bytes, data(&claimed, 1));
        assert!(!f.root.join(fixture::legacy_name(&claimed, 0)).exists());
        healthy(&f, &receiver, 2).await;
    })
    .await;
}
