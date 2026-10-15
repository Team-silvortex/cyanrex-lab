use super::fixture::{directory, fingerprint, mode, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, verify_platform_artifact_root,
    PlatformHostPreflightError as Error,
};
use std::{fs, os::unix::fs::MetadataExt};

#[test]
fn platform_artifact_root_requires_exact_private_mode_and_preserves_rejected_entries() {
    let results = Fixture::checked(|fixture| {
        let observation = inspect_platform_artifact_root(&fixture.config, &fixture.root)
            .unwrap_or_else(|_| panic!("original private root observation unavailable"));
        let mut results = Vec::new();
        for bits in [
            0o000, 0o500, 0o600, 0o710, 0o750, 0o755, 0o770, 0o777, 0o1700, 0o2700, 0o4700,
        ] {
            mode(&fixture.root, bits);
            let before = fingerprint(&fixture.root);
            let inspected =
                inspect_platform_artifact_root(&fixture.config, &fixture.root).map(|_| ());
            let verified =
                verify_platform_artifact_root(&fixture.config, &fixture.root, &observation);
            let unchanged = before == fingerprint(&fixture.root);
            mode(&fixture.root, 0o700);
            let restored =
                verify_platform_artifact_root(&fixture.config, &fixture.root, &observation);
            let fresh = inspect_platform_artifact_root(&fixture.config, &fixture.root);
            let same = fresh.as_ref().is_ok_and(|fresh| observation.matches(fresh));
            results.push((inspected, verified, unchanged, restored, same));
        }
        drop(observation);
        results
    });
    assert_eq!(results.len(), 11);
    for (inspected, verified, unchanged, restored, same) in results {
        assert_eq!(inspected, Err(Error::ArtifactRootUnavailable));
        assert_eq!(verified, Err(Error::ArtifactRootChanged));
        assert!(unchanged);
        assert_eq!(restored, Ok(()));
        assert!(
            same,
            "restoration of original security fields is not invalidated by ctime"
        );
    }
}

#[test]
fn platform_artifact_root_rechecks_unsafe_ancestors_without_pinning_ancestor_identity() {
    let (results, replacement, unchanged) = Fixture::checked(|fixture| {
        let observation = inspect_platform_artifact_root(&fixture.config, &fixture.root)
            .unwrap_or_else(|_| panic!("original root observation unavailable"));
        let mut results = Vec::new();
        for bits in [0o722, 0o770, 0o777] {
            mode(fixture.directory(), bits);
            let before = fingerprint(fixture.directory());
            let inspected =
                inspect_platform_artifact_root(&fixture.config, &fixture.root).map(|_| ());
            let verified =
                verify_platform_artifact_root(&fixture.config, &fixture.root, &observation);
            let unchanged = before == fingerprint(fixture.directory());
            mode(fixture.directory(), 0o700);
            let restored =
                verify_platform_artifact_root(&fixture.config, &fixture.root, &observation);
            results.push((inspected, verified, unchanged, restored));
        }
        drop(observation);

        let ancestor = fixture.directory().join("safe-ancestor");
        let retained = fixture.directory().join("retained-ancestor");
        directory(&ancestor);
        let root = ancestor.join("artifacts");
        directory(&root);
        let original_ancestor = fs::symlink_metadata(&ancestor)
            .unwrap_or_else(|_| panic!("synthetic ancestor identity unavailable"))
            .ino();
        let observation = inspect_platform_artifact_root(&fixture.config, &root)
            .unwrap_or_else(|_| panic!("nested private root observation unavailable"));
        fs::rename(&ancestor, &retained).unwrap_or_else(|_| panic!("owned ancestor rename failed"));
        directory(&ancestor);
        fs::rename(retained.join("artifacts"), &root)
            .unwrap_or_else(|_| panic!("same-inode root placement setup failed"));
        let replaced_ancestor = fs::symlink_metadata(&ancestor)
            .unwrap_or_else(|_| panic!("replacement ancestor identity unavailable"))
            .ino();
        let before = fingerprint(&root);
        let verified = verify_platform_artifact_root(&fixture.config, &root, &observation);
        let fresh = inspect_platform_artifact_root(&fixture.config, &root);
        let same = fresh.as_ref().is_ok_and(|fresh| observation.matches(fresh));
        let unchanged = before == fingerprint(&root);
        drop(fresh);
        drop(observation);
        (
            results,
            (verified, same, original_ancestor != replaced_ancestor),
            unchanged,
        )
    });
    assert_eq!(results.len(), 3);
    for (inspected, verified, unchanged, restored) in results {
        assert_eq!(inspected, Err(Error::ArtifactRootUnavailable));
        assert_eq!(verified, Err(Error::ArtifactRootChanged));
        assert!(unchanged);
        assert_eq!(restored, Ok(()));
    }
    assert_eq!(replacement.0, Ok(()));
    assert!(
        replacement.1 && replacement.2 && unchanged,
        "only the terminal identity is pinned; currently safe ancestors are re-walked"
    );
}
