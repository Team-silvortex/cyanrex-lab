use super::fixture::Fixture;
use cyanrex_engine::platform_host::{
    inspect_platform_artifact_root, verify_platform_artifact_root,
};

#[test]
fn platform_artifact_root_observes_existing_nonempty_private_directory_without_mutation() {
    let (first, second, matches, verified, unchanged, config_unchanged) =
        Fixture::checked(|fixture| {
            let before = fixture.snapshot();
            let config = fixture.config.clone();
            let first = inspect_platform_artifact_root(&fixture.config, &fixture.root);
            let second = inspect_platform_artifact_root(&fixture.config, &fixture.root);
            let matches = match (&first, &second) {
                (Ok(first), Ok(second)) => first.matches(second) && second.matches(first),
                _ => false,
            };
            let verified = match &first {
                Ok(observation) => {
                    verify_platform_artifact_root(&fixture.config, &fixture.root, observation)
                }
                Err(error) => Err(*error),
            };
            // Both success observations are dropped before the checked fixture removes its root.
            let first = first.map(|_| ());
            let second = second.map(|_| ());
            let unchanged = before == fixture.snapshot();
            (
                first,
                second,
                matches,
                verified,
                unchanged,
                fixture.config == config,
            )
        });
    // The inert implementation must fail here, never before cleanup due to a missing API or setup.
    assert!(
        unchanged,
        "inspection must not alter the root, parent or existing marker"
    );
    assert!(config_unchanged);
    assert_eq!(first, Ok(()));
    assert_eq!(second, Ok(()));
    assert!(matches);
    assert_eq!(verified, Ok(()));
}
