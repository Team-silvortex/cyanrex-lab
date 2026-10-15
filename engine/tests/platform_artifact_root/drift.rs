use super::fixture::{directory, fingerprint, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, verify_platform_artifact_root,
    PlatformHostPreflightError as Error,
};
use std::{fs, os::unix::fs::symlink};

#[test]
fn platform_artifact_root_rejects_replaced_roots_and_recovers_only_the_original_inode() {
    let results = Fixture::checked(|fixture| {
        let original = inspect_platform_artifact_root(&fixture.config, &fixture.root)
            .unwrap_or_else(|_| panic!("original root observation unavailable"));
        let mut results = Vec::new();
        for replacement in 0..3 {
            let retained = fixture.directory().join("retained-original-root");
            fs::rename(&fixture.root, &retained)
                .unwrap_or_else(|_| panic!("owned root displacement failed"));
            if replacement == 0 {
                directory(&fixture.root);
            }
            if replacement == 1 {
                symlink(&retained, &fixture.root)
                    .unwrap_or_else(|_| panic!("synthetic root symlink setup failed"));
            }
            let before = fingerprint(&retained);
            let visible_before = (replacement < 2).then(|| fingerprint(&fixture.root));
            let verified = verify_platform_artifact_root(&fixture.config, &fixture.root, &original);
            let fresh = inspect_platform_artifact_root(&fixture.config, &fixture.root);
            let new_root_not_adopted = match &fresh {
                Ok(fresh) => replacement == 0 && !original.matches(fresh),
                Err(error) => replacement != 0 && *error == Error::ArtifactRootUnavailable,
            };
            let unchanged = before == fingerprint(&retained)
                && visible_before.is_none_or(|before| before == fingerprint(&fixture.root));
            drop(fresh);
            if replacement == 0 {
                fs::remove_dir(&fixture.root)
                    .unwrap_or_else(|_| panic!("owned empty replacement cleanup failed"));
            } else if replacement == 1 {
                fs::remove_file(&fixture.root)
                    .unwrap_or_else(|_| panic!("owned synthetic alias cleanup failed"));
            }
            fs::rename(&retained, &fixture.root)
                .unwrap_or_else(|_| panic!("original root restoration failed"));
            let restored = verify_platform_artifact_root(&fixture.config, &fixture.root, &original);
            results.push((verified, new_root_not_adopted, unchanged, restored));
        }
        drop(original);
        results
    });
    assert_eq!(results.len(), 3);
    for (verified, not_adopted, unchanged, restored) in results {
        assert_eq!(verified, Err(Error::ArtifactRootChanged));
        assert!(not_adopted && unchanged);
        assert_eq!(restored, Ok(()));
    }
}
