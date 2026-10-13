use super::super::assembly_test_support::Root;
use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn user_directory_cached_gid_mismatch_rejects_without_claiming_an_actual_group_change() {
    let parent = Root::new();
    fs::set_permissions(&parent.0, fs::Permissions::from_mode(0o755)).unwrap();
    let actual = fs::metadata(&parent.0).unwrap();
    let SaveDirectory::User(mut directory) = SaveDirectory::open_user(&parent.0).unwrap() else {
        panic!("the explicit policy must capture a user directory");
    };
    directory.verify_root().unwrap();
    directory.original.gid ^= 1;
    assert!(
        directory.verify_root().is_err(),
        "compare the original cached GID as well as UID/mode"
    );
    let current = fs::metadata(&parent.0).unwrap();
    assert_eq!(
        (
            current.dev(),
            current.ino(),
            current.uid(),
            current.gid(),
            current.mode()
        ),
        (
            actual.dev(),
            actual.ino(),
            actual.uid(),
            actual.gid(),
            actual.mode()
        )
    );
}
