use super::fixture::{directory, fingerprint, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, PlatformHostPreflightError as Error,
};
use std::{
    ffi::{CString, OsString},
    fs,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::symlink,
    },
    path::PathBuf,
};

#[test]
fn platform_artifact_root_rejects_invalid_or_missing_paths_without_creation() {
    let (results, unchanged) = Fixture::checked(|fixture| {
        let before = fixture.snapshot();
        let paths = [
            PathBuf::new(),
            PathBuf::from("relative"),
            PathBuf::from("/"),
            fixture.root.join("..").join("private-artifacts"),
            fixture.directory().join("missing"),
            fixture.root.join("missing-child"),
            fixture.directory().join("x".repeat(4097)),
            fixture.root.join(OsString::from_vec(vec![b'n', 0, b'l'])),
        ];
        let results: Vec<_> = paths
            .iter()
            .map(|path| inspect_platform_artifact_root(&fixture.config, path).map(|_| ()))
            .collect();
        (results, before == fixture.snapshot())
    });
    assert!(unchanged);
    assert_eq!(results.len(), 8);
    for result in results {
        assert_eq!(result, Err(Error::ArtifactRootUnavailable));
    }
}

#[test]
fn platform_artifact_root_rejects_symlink_and_special_directory_components_without_following() {
    let (results, unchanged) = Fixture::checked(|fixture| {
        directory(&fixture.root.join("nested"));
        let alias = fixture.directory().join("root-alias");
        let dangling = fixture.directory().join("dangling-alias");
        symlink(&fixture.root, &alias)
            .unwrap_or_else(|_| panic!("synthetic root alias setup failed"));
        symlink(fixture.directory().join("missing-target"), &dangling)
            .unwrap_or_else(|_| panic!("synthetic dangling alias setup failed"));
        let pipe = fixture.directory().join("root-fifo");
        let name = CString::new(pipe.as_os_str().as_bytes()).unwrap();
        // SAFETY: the exclusive fixture owns this bounded NUL-terminated synthetic pathname.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let before = fixture.snapshot();
        let pipe_before = fingerprint(&pipe);
        let paths = [
            alias.clone(),
            alias.join("nested"),
            dangling,
            fixture.marker().to_owned(),
            fixture.marker().join("child"),
            pipe.clone(),
            pipe.join("child"),
        ];
        let results: Vec<_> = paths
            .iter()
            .map(|path| inspect_platform_artifact_root(&fixture.config, path).map(|_| ()))
            .collect();
        let unchanged = before == fixture.snapshot()
            && pipe_before == fingerprint(&pipe)
            && fs::read_link(&alias).is_ok_and(|target| target == fixture.root);
        (results, unchanged)
    });
    assert!(
        unchanged,
        "no alias target, marker or special entry may be followed or repaired"
    );
    assert_eq!(results.len(), 7);
    for result in results {
        assert_eq!(result, Err(Error::ArtifactRootUnavailable));
    }
}
