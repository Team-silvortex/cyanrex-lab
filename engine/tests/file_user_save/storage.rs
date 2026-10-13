use crate::{fixture::*, support::*, *};
use std::{
    ffi::CString,
    fs::{self, OpenOptions},
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, OpenOptionsExt},
    },
    time::Duration,
};

#[tokio::test]
async fn valid_source_and_ordinary_directory_never_overwrite_regular_empty_directory_symlink_dangling_hardlink_or_fifo_entries(
) {
    for attack in 0..7 {
        checked(Fixture::with_length(17), |f| {
            Box::pin(async move {
                let assembled = f.assemble().await;
                let selected = selected_directory(f);
                let target = FileSaveTarget::open_user_directory(&selected, CHOSEN_LEAF).unwrap();
                let path = selected.join(CHOSEN_LEAF);
                let marker = f.directory().join("external-entry-marker");
                write_private(&marker, b"never adopted or overwritten");
                match attack {
                    0 => write_private(&path, b"existing selected output"),
                    1 => write_private(&path, b""),
                    2 => directory(&path, 0o755),
                    3 => symlink(&marker, &path).unwrap(),
                    4 => symlink(f.directory().join("missing-marker"), &path).unwrap(),
                    5 => fs::hard_link(&marker, &path).unwrap(),
                    6 => {
                        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
                        // SAFETY: only this exclusive fixture's explicit selected entry is affected.
                        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
                    }
                    _ => unreachable!(),
                }
                let source = bytes::tree(&f.source_root);
                let entries = bytes::tree(&selected);
                let marker_before = bytes::snapshot(&marker);
                let parent = directory_policy(&selected);
                let result = tokio::time::timeout(
                    Duration::from_secs(3),
                    f.store
                        .save_assembled_file(&f.manifest, &assembled, &target),
                )
                .await;
                let rescue = if attack == 6 && result.is_err() {
                    // Drain a regressed blocking FIFO open before propagating failure/cleanup.
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(&path)
                        .unwrap();
                    restored(f).await;
                    Some(file)
                } else {
                    None
                };
                drop(rescue);
                assert_eq!(
                    result
                        .expect("existing entry rejection must not block")
                        .err(),
                    Some(FileSaveError::Conflict)
                );
                assert_eq!(bytes::tree(&selected), entries);
                assert_eq!(bytes::tree(&f.source_root), source);
                assert_eq!(bytes::snapshot(&marker), marker_before);
                assert_eq!(directory_policy(&selected), parent);
                assert!(!f.directory().join("missing-marker").exists());
            })
        })
        .await;
    }
}
