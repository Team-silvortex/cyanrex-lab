//! Internal capability split, never public storage duplication or a newly observed baseline.
use super::{
    PlatformLoginConnector, PlatformSessionConnector, PlatformStorageConnector,
    PlatformTaskContentConnector, PlatformTaskCreateConnector, PlatformTaskIndexConnector,
    PlatformTaskReplaceConnector, PlatformTaskTextArtifactConnector, PlatformTextArtifactConnector,
};
use std::sync::Arc;

pub(crate) fn split_platform_session_connectors(
    storage: PlatformStorageConnector,
) -> (PlatformLoginConnector, PlatformSessionConnector) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(storage),
    )
}

#[allow(dead_code)]
pub(crate) fn split_platform_workspace_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(storage),
    )
}

#[allow(dead_code)]
pub(crate) fn split_platform_content_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
    PlatformTaskContentConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskContentConnector::from_shared(storage),
    )
}

#[allow(dead_code)]
pub(crate) fn split_platform_task_create_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
    PlatformTaskContentConnector,
    PlatformTaskCreateConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskContentConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskCreateConnector::from_shared(storage),
    )
}

pub(crate) fn split_platform_text_publication_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
    PlatformTaskContentConnector,
    PlatformTaskCreateConnector,
    PlatformTaskReplaceConnector,
    PlatformTextArtifactConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskContentConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskCreateConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskReplaceConnector::from_shared(Arc::clone(&storage)),
        PlatformTextArtifactConnector::from_shared(storage),
    )
}

pub(crate) fn split_platform_task_replace_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
    PlatformTaskContentConnector,
    PlatformTaskCreateConnector,
    PlatformTaskReplaceConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskContentConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskCreateConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskReplaceConnector::from_shared(storage),
    )
}

pub(crate) fn split_platform_task_text_publication_connectors(
    storage: PlatformStorageConnector,
) -> (
    PlatformLoginConnector,
    PlatformSessionConnector,
    PlatformTaskIndexConnector,
    PlatformTaskContentConnector,
    PlatformTaskCreateConnector,
    PlatformTaskReplaceConnector,
    PlatformTextArtifactConnector,
    PlatformTaskTextArtifactConnector,
) {
    let storage = Arc::new(storage);
    (
        PlatformLoginConnector::from_shared(Arc::clone(&storage)),
        PlatformSessionConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskIndexConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskContentConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskCreateConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskReplaceConnector::from_shared(Arc::clone(&storage)),
        PlatformTextArtifactConnector::from_shared(Arc::clone(&storage)),
        PlatformTaskTextArtifactConnector::from_shared(storage),
    )
}
