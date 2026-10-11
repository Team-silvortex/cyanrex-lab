use super::{bytes::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStagingError, file_upload_staging::UploadFileVerificationError,
};
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions, Permissions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, OpenOptionsExt, PermissionsExt},
    },
    time::Duration,
};

async fn fifo_observation(fixture: &Fixture, path: &std::path::Path) -> bool {
    let rows = fixture.rows().await;
    let oids = fixture.oids().await;
    let files = tree(&fixture.root);
    let mut pending = Box::pin(fixture.adapter.verify_file(upload(1), owner(1), account(1)));
    let result = tokio::time::timeout(Duration::from_secs(2), &mut pending).await;
    let on_time = result.is_ok();
    let mut rescue: Option<File> = None;
    if on_time {
        assert_eq!(
            result.unwrap().unwrap_err(),
            UploadFileVerificationError::Unavailable
        );
    } else {
        // Only a broken blocking open gets a writer. Keeping it until real slot recovery avoids
        // leaving an infinite blocking worker behind when the diagnostic assertion later fails.
        rescue = Some(
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)
                .unwrap(),
        );
    }
    drop(pending);
    if !on_time {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match fixture
                    .staging
                    .verify_staged_chunk(&manifest(17, 900), 0)
                    .await
                {
                    Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                    Err(ChunkStagingError::Unavailable) => break,
                    other => panic!("unexpected restoration probe: {other:?}"),
                }
            }
        })
        .await
        .expect("FIFO rescue did not release the actual single worker");
    }
    drop(rescue);
    assert_eq!(fixture.rows().await, rows);
    assert_eq!(fixture.oids().await, oids);
    assert_eq!(tree(&fixture.root), files);
    on_time
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_missing_corrupt_special_or_nonprivate_chunks_fail_without_repairs() {
    for damage in [
        "missing",
        "corrupt",
        "short",
        "long",
        "symlink",
        "hardlink",
        "fifo",
        "directory",
        "mode",
        "root_mode",
    ] {
        let fixture = Fixture::new(4, 1).await;
        fixture
            .source
            .reserve(upload(1), owner(1), account(1), &fixture.manifest)
            .await
            .unwrap();
        stage_all(&fixture).await;
        good(&fixture, &fixture.adapter).await;
        let entries = tree(&fixture.root);
        let path = fixture.root.join(&entries[0].name);
        let saved = fixture.directory.join("saved-original");
        let mut substituted = false;
        match damage {
            "missing" | "symlink" | "hardlink" | "fifo" | "directory" => {
                fs::rename(&path, &saved).unwrap();
                substituted = true;
                match damage {
                    "missing" => {}
                    "symlink" => symlink(&saved, &path).unwrap(),
                    "hardlink" => fs::hard_link(&saved, &path).unwrap(),
                    "fifo" => {
                        let raw = CString::new(path.as_os_str().as_bytes()).unwrap();
                        // SAFETY: raw is a live NUL-terminated exclusive fixture pathname.
                        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
                    }
                    "directory" => fs::create_dir(&path).unwrap(),
                    _ => unreachable!(),
                }
            }
            "corrupt" | "short" | "long" => {
                fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
                let length = if damage == "short" {
                    16
                } else if damage == "long" {
                    18
                } else {
                    17
                };
                let mut bytes = Vec::new();
                Pattern::new(0, length).read_to_end(&mut bytes).unwrap();
                if damage == "corrupt" {
                    bytes[0] ^= 1;
                }
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&bytes)
                    .unwrap();
                fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            }
            "mode" => fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap(),
            "root_mode" => {
                fs::set_permissions(&fixture.root, Permissions::from_mode(0o770)).unwrap()
            }
            _ => unreachable!(),
        }
        let saved_before = substituted.then(|| snapshot(&saved));
        let on_time = if damage == "fifo" {
            fifo_observation(&fixture, &path).await
        } else {
            bad(&fixture, UploadFileVerificationError::Unavailable).await;
            true
        };
        if let Some(saved_before) = saved_before {
            assert_eq!(
                snapshot(&saved),
                saved_before,
                "verification modified the original outside the root"
            );
        }
        if substituted {
            if damage == "directory" {
                fs::remove_dir(&path).unwrap();
            } else if damage != "missing" {
                fs::remove_file(&path).unwrap();
            }
            fs::rename(&saved, &path).unwrap();
        } else if ["corrupt", "short", "long"].contains(&damage) {
            fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut Pattern::new(0, 17), &mut file).unwrap();
            drop(file);
            fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
        } else if damage == "mode" {
            fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
        } else if damage == "root_mode" {
            fs::set_permissions(&fixture.root, Permissions::from_mode(0o700)).unwrap();
        }
        good(&fixture, &fixture.adapter).await;
        fixture.pool_restored().await;
        fixture.cleanup().await;
        assert!(
            on_time,
            "no-writer FIFO must be rejected without waiting for an anchor"
        );
    }
}
