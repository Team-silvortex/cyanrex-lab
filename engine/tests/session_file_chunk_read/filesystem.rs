use crate::{fixture::Fixture, support::*, *};
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions, Permissions},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, DirBuilderExt, OpenOptionsExt, PermissionsExt},
    },
    time::Duration,
};
async fn fifo_read(f: &Fixture, path: &std::path::Path) -> bool {
    let before = f.sharing.snapshot().await;
    let r = f.request();
    let mut pending = Box::pin(auth(f).source.read_session_file_chunk(
        &auth(f).target_token,
        &f.workspace,
        &r,
    ));
    let completed = tokio::time::timeout(Duration::from_secs(2), &mut pending).await;
    let on_time = completed.is_ok();
    let mut rescue: Option<File> = None;
    if on_time {
        assert_eq!(
            completed.unwrap().err(),
            Some(SessionFileChunkReadError::Unavailable)
        );
    } else {
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
                match auth(f)
                    .source
                    .read_session_file_chunk(&auth(f).target_token, &f.workspace, &r)
                    .await
                {
                    Err(SessionFileChunkReadError::CapacityExhausted) => {
                        tokio::task::yield_now().await
                    }
                    Err(SessionFileChunkReadError::Unavailable) => break,
                    other => panic!("unexpected FIFO restoration probe: {other:?}"),
                }
            }
        })
        .await
        .expect("rescue writer must release the broken blocking open's actual byte lease");
    }
    drop(rescue);
    assert_eq!(f.sharing.snapshot().await, before);
    on_time
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_rejects_corrupt_linked_special_or_wrong_bound_files_without_disclosure_or_repair(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
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
                "legacy",
            ] {
                healthy(f).await;
                let entries = bytes::tree(&f.sharing.base.root);
                let path = f.sharing.base.root.join(&entries[0].name);
                let saved = f.sharing.base.directory.join("saved-original");
                let mut substituted = false;
                let mut legacy = None;
                match damage {
                    "missing" | "symlink" | "hardlink" | "fifo" | "directory" | "legacy" => {
                        fs::rename(&path, &saved).unwrap();
                        substituted = true;
                        match damage {
                            "symlink" => symlink(&saved, &path).unwrap(),
                            "hardlink" => fs::hard_link(&saved, &path).unwrap(),
                            "fifo" => {
                                let raw = CString::new(path.as_os_str().as_bytes()).unwrap();
                                // SAFETY: raw is a live NUL-terminated exclusive synthetic path.
                                assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
                            }
                            "directory" => fs::create_dir(&path).unwrap(),
                            "legacy" => {
                                f.staging
                                    .stage_chunk(
                                        &f.sharing.base.manifest,
                                        0,
                                        bytes::Pattern::new(0, 17),
                                    )
                                    .await
                                    .unwrap();
                                legacy = bytes::tree(&f.sharing.base.root)
                                    .into_iter()
                                    .find(|e| e.name.to_string_lossy().starts_with("v1-"))
                                    .map(|e| f.sharing.base.root.join(e.name));
                                assert!(legacy.is_some());
                            }
                            _ => {}
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
                        let mut content = (0..length).map(bytes::byte).collect::<Vec<_>>();
                        if damage == "corrupt" {
                            content[0] ^= 1;
                        }
                        OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .open(&path)
                            .unwrap()
                            .write_all(&content)
                            .unwrap();
                        fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
                    }
                    "mode" => fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap(),
                    _ => fs::set_permissions(&f.sharing.base.root, Permissions::from_mode(0o770))
                        .unwrap(),
                }
                let saved_before = substituted.then(|| bytes::snapshot(&saved));
                let on_time = if damage == "fifo" {
                    fifo_read(f, &path).await
                } else {
                    assert_eq!(
                        read(f, &f.request()).await,
                        Err(SessionFileChunkReadError::Unavailable),
                        "{damage} must return NO byte DTO"
                    );
                    true
                };
                if let Some(saved_before) = saved_before {
                    assert_eq!(
                        bytes::snapshot(&saved),
                        saved_before,
                        "read must not alter the displaced original object"
                    );
                }
                if let Some(legacy) = legacy {
                    fs::remove_file(legacy).unwrap();
                }
                if substituted {
                    if damage == "directory" {
                        fs::remove_dir(&path).unwrap();
                    } else if ["symlink", "hardlink", "fifo"].contains(&damage) {
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
                    std::io::copy(&mut bytes::Pattern::new(0, 17), &mut file).unwrap();
                    drop(file);
                    fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
                } else if damage == "mode" {
                    fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
                } else if damage == "root_mode" {
                    fs::set_permissions(&f.sharing.base.root, Permissions::from_mode(0o700))
                        .unwrap();
                }
                healthy(f).await;
                assert!(on_time, "FIFO must reject without any writer anchor");
            }
            let other = f.sharing.base.directory.join("another-private-root");
            fs::DirBuilder::new().mode(0o700).create(&other).unwrap();
            let downloads = auth(f)
                .source
                .file_download_workspace(
                    f.sharing.base.publication.clone(),
                    f.sharing.recipient_workspace.clone(),
                )
                .unwrap();
            let wrong = auth(f)
                .source
                .file_chunk_read_workspace(
                    downloads,
                    ChunkStaging::open(scope(), &other, 1).unwrap(),
                    1,
                )
                .unwrap();
            assert_eq!(
                read_as(
                    f,
                    &auth(f).source,
                    &auth(f).target_token,
                    &wrong,
                    &f.request()
                )
                .await,
                Err(denied()),
                "another root incarnation cannot supply an exact publication's bytes"
            );
            healthy(f).await;
            drain(f).await;
        })
    })
    .await;
}
