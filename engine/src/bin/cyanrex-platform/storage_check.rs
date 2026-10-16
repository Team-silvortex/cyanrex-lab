use super::config::{StorageExpectation, ValidatedInput};
use cyanrex_engine::platform_host::{
    check_platform_database_storage, check_platform_database_storage_expected,
    check_platform_database_storage_markers_expected, inspect_platform_artifact_root,
    verify_platform_artifact_root, PlatformProcessStartup,
};

pub(super) fn check(input: ValidatedInput, process: &PlatformProcessStartup) -> Result<(), ()> {
    process.verify_before_runtime().map_err(|_| ())?;
    // Synchronous existing-root metadata checks precede runtime creation and database dialing.
    // They have no hard time bound, enumerate no content, and never install or repair a root.
    let original =
        inspect_platform_artifact_root(&input.config, &input.artifact_root).map_err(|_| ())?;
    let inspected = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => {
            let result = runtime
                .block_on(async {
                    match &input.storage_expectation {
                        StorageExpectation::Markers(expected) => {
                            check_platform_database_storage_markers_expected(
                                &input.config,
                                &input.endpoint,
                                &input.password,
                                expected,
                            )
                            .await
                        }
                        StorageExpectation::Catalog(expected) => {
                            check_platform_database_storage_expected(
                                &input.config,
                                &input.endpoint,
                                &input.password,
                                expected,
                            )
                            .await
                        }
                        StorageExpectation::Current => {
                            check_platform_database_storage(
                                &input.config,
                                &input.endpoint,
                                &input.password,
                            )
                            .await
                        }
                    }
                })
                .map_err(|_| ());
            drop(runtime);
            result
        }
        Err(_) => Err(()),
    };
    // Recheck both even when database work failed; no success observation escapes this command.
    // These distinct snapshots are not one cross-resource atomic transaction or a future pin.
    let root = verify_platform_artifact_root(&input.config, &input.artifact_root, &original)
        .map_err(|_| ());
    let process = process.verify_before_runtime().map_err(|_| ());
    inspected.and(root).and(process)
}
