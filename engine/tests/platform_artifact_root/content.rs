use super::fixture::{directory, mode, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, verify_platform_artifact_root,
    PlatformHostPreflightError as Error,
};
use std::fs;

#[test]
fn platform_artifact_root_ignores_content_changes_and_keeps_matches_passive() {
    let (results, matches_while_missing, missing, restored, unchanged) =
        Fixture::checked(|fixture| {
            let first = inspect_platform_artifact_root(&fixture.config, &fixture.root)
                .unwrap_or_else(|_| panic!("first root observation unavailable"));
            let second = inspect_platform_artifact_root(&fixture.config, &fixture.root)
                .unwrap_or_else(|_| panic!("second root observation unavailable"));
            let extra = fixture.root.join("ordinary-unpublished-content.bin");
            let child = fixture.root.join("ordinary-child-directory");
            fs::write(
                &extra,
                b"synthetic mutable content, not validated Artifact bytes",
            )
            .unwrap_or_else(|_| panic!("synthetic ordinary entry creation failed"));
            directory(&child);
            let mut results = Vec::new();
            for phase in 0..3 {
                if phase == 1 {
                    fs::remove_file(&extra)
                        .unwrap_or_else(|_| panic!("owned ordinary entry removal failed"));
                    fs::remove_dir(&child)
                        .unwrap_or_else(|_| panic!("owned ordinary child removal failed"));
                } else if phase == 2 {
                    mode(fixture.marker(), 0o600);
                    fs::write(
                        fixture.marker(),
                        b"synthetic changed marker; preflight does not validate file content",
                    )
                    .unwrap_or_else(|_| panic!("synthetic marker change failed"));
                    mode(fixture.marker(), 0o400);
                }
                let before = fixture.snapshot();
                let verified =
                    verify_platform_artifact_root(&fixture.config, &fixture.root, &first);
                let fresh = inspect_platform_artifact_root(&fixture.config, &fixture.root);
                let same = fresh
                    .as_ref()
                    .is_ok_and(|fresh| first.matches(fresh) && fresh.matches(&first));
                results.push((
                    verified,
                    fresh.map(|_| ()),
                    same,
                    before == fixture.snapshot(),
                ));
            }
            let retained = fixture.directory().join("temporarily-hidden-original");
            fs::rename(&fixture.root, &retained)
                .unwrap_or_else(|_| panic!("owned root displacement failed"));
            let matches_while_missing = first.matches(&second) && second.matches(&first);
            let missing = verify_platform_artifact_root(&fixture.config, &fixture.root, &first);
            fs::rename(&retained, &fixture.root)
                .unwrap_or_else(|_| panic!("same-inode root restoration failed"));
            let before = fixture.snapshot();
            let restored = verify_platform_artifact_root(&fixture.config, &fixture.root, &first);
            let unchanged = before == fixture.snapshot();
            drop(first);
            drop(second);
            (results, matches_while_missing, missing, restored, unchanged)
        });
    assert_eq!(results.len(), 3);
    for (verified, fresh, same, unchanged) in results {
        assert_eq!(verified, Ok(()));
        assert_eq!(fresh, Ok(()));
        assert!(same && unchanged);
    }
    assert!(
        matches_while_missing,
        "matches compares cached observations, not filesystem health"
    );
    assert_eq!(missing, Err(Error::ArtifactRootChanged));
    assert_eq!(restored, Ok(()));
    assert!(unchanged);
}
