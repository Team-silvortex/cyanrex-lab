use super::super::contract::MapOnly;
use crate::models::collaboration::{
    CoreSchemaVersion, QualifiedName, RevisionNumber, TaskContentManifest, TaskDefinitionRef,
    TaskId, VersionedName,
};
use serde::Deserialize;

/// Only an exact metadata reference crosses this boundary, never a caller-supplied definition.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    pub schema_version: CoreSchemaVersion,
    pub task_id: TaskId,
    pub definition_ref: MapOnly<DefinitionRef>,
    pub manifest: TaskContentManifest,
}

// MapOnly applies independently to both records. Direct streaming into strict derives preserves
// duplicate keys and rejects positional arrays without changing the shared model's wire contract.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefinitionRef {
    package: MapOnly<VersionedName>,
    name: QualifiedName,
    version: RevisionNumber,
}

impl DefinitionRef {
    pub(super) fn into_reference(self) -> TaskDefinitionRef {
        TaskDefinitionRef {
            package: self.package.0,
            name: self.name,
            version: self.version,
        }
    }
}
