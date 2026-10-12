use crate::{fixture::*, *};
use std::{
    fs,
    io::Write,
    os::unix::fs::{symlink, PermissionsExt},
};

#[tokio::test]
async fn missing_corrupt_short_extra_and_writable_inputs_are_rejected_without_side_effects() {
    for kind in ["missing", "digest", "short", "extra", "writable"] {
        let f = Fixture::staged(17).await;
        let path = f.root.join(chunk_name(&f.manifest, 0));
        let original = f.directory.join("retained-original");
        fs::rename(&path, &original).unwrap();
        if kind != "missing" {
            fs::copy(&original, &path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
            match kind {
                "digest" => file.write_all(&[255]).unwrap(),
                "short" => file.set_len(16).unwrap(),
                "extra" => file.set_len(18).unwrap(),
                "writable" => (),
                _ => unreachable!(),
            }
            drop(file);
            if kind != "writable" {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            }
        }
        let before = bytes::tree(&f.root);
        let source = bytes::snapshot(&f.source);
        let retained = bytes::snapshot(&original);
        let result = f.staging.assemble_staged_file(&f.manifest).await;
        assert!(
            matches!(result, Err(ChunkStagingError::Unavailable)),
            "{kind}"
        );
        assert_eq!(bytes::tree(&f.root), before, "{kind}");
        assert_eq!(bytes::snapshot(&f.source), source);
        assert_eq!(bytes::snapshot(&original), retained);
        if kind != "missing" {
            fs::remove_file(&path).unwrap();
        }
        fs::rename(&original, &path).unwrap();
        healthy(&f).await;
        f.cleanup();
    }
}

#[tokio::test]
async fn linked_and_nonregular_inputs_never_follow_block_or_modify_external_evidence() {
    for kind in ["symlink", "hardlink", "fifo", "directory"] {
        let f = Fixture::staged(17).await;
        let path = f.root.join(chunk_name(&f.manifest, 0));
        let original = f.directory.join("original-chunk");
        fs::rename(&path, &original).unwrap();
        match kind {
            "symlink" => symlink(&f.source, &path).unwrap(),
            "hardlink" => fs::hard_link(&original, &path).unwrap(),
            "fifo" => fifo(&path),
            "directory" => fs::create_dir(&path).unwrap(),
            _ => unreachable!(),
        }
        let before = bytes::tree(&f.root);
        let source = bytes::snapshot(&f.source);
        let retained = bytes::snapshot(&original);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            f.staging.assemble_staged_file(&f.manifest),
        )
        .await
        .unwrap();
        assert!(
            matches!(result, Err(ChunkStagingError::Unavailable)),
            "{kind}"
        );
        assert_eq!(bytes::tree(&f.root), before);
        assert_eq!(bytes::snapshot(&f.source), source);
        assert_eq!(bytes::snapshot(&original), retained);
        if kind == "directory" {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        fs::rename(&original, &path).unwrap();
        healthy(&f).await;
        f.cleanup();
    }
}

#[tokio::test]
async fn every_existing_output_type_conflicts_even_when_input_is_now_missing() {
    for kind in [
        "correct",
        "corrupt",
        "symlink",
        "dangling",
        "hardlink",
        "fifo",
        "directory",
    ] {
        let f = Fixture::staged(17).await;
        let path = f.root.join(output_name(&f.manifest));
        match kind {
            "correct" => fs::copy(&f.source, &path).map(|_| ()).unwrap(),
            "corrupt" => fs::write(&path, b"different existing file").unwrap(),
            "symlink" => symlink(&f.source, &path).unwrap(),
            "dangling" => symlink(f.directory.join("missing-target"), &path).unwrap(),
            "hardlink" => fs::hard_link(&f.source, &path).unwrap(),
            "fifo" => fifo(&path),
            "directory" => fs::create_dir(&path).unwrap(),
            _ => unreachable!(),
        }
        if ["correct", "corrupt"].contains(&kind) {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        let input = f.root.join(chunk_name(&f.manifest, 0));
        let retained_path = f.directory.join("retained-input");
        fs::rename(&input, &retained_path).unwrap();
        let before = bytes::tree(&f.root);
        let source = bytes::snapshot(&f.source);
        let retained = bytes::snapshot(&retained_path);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            f.staging.assemble_staged_file(&f.manifest),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(ChunkStagingError::Conflict)), "{kind}");
        assert_eq!(bytes::tree(&f.root), before);
        assert_eq!(bytes::snapshot(&f.source), source);
        assert_eq!(bytes::snapshot(&retained_path), retained);
        if kind == "directory" {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        fs::rename(&retained_path, &input).unwrap();
        healthy(&f).await;
        f.cleanup();
    }
}
