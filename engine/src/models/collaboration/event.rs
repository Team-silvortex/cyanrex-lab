use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    AggregateRef, ArtifactRef, CoreSchemaVersion, CorrelationId, EventId, PrincipalRef,
    QualifiedName, RevisionNumber, WorkspaceRef,
};

/// A proposed business-event record, separate from the existing lossy telemetry Event/EventBus.
/// It provides neither persistence nor an authenticated audit trail by merely being deserializable.
/// Unknown historical facts stay None; recorded_at must be supplied by the recorder, not invented
/// during decoding. Stream offsets belong to the future durable transport, not to this envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub schema_version: CoreSchemaVersion,
    pub event_id: EventId,
    pub workspace: WorkspaceRef,
    pub aggregate: AggregateRef,
    pub aggregate_revision: RevisionNumber,
    pub actor: Option<PrincipalRef>,
    pub event_type: QualifiedName,
    pub occurred_at: Option<DateTime<Utc>>,
    pub recorded_at: DateTime<Utc>,
    pub correlation_id: CorrelationId,
    pub causation_id: Option<EventId>,
    pub payload_ref: Option<ArtifactRef>,
}
