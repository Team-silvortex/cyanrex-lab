-- Explicit artifact staging only. Never install into a live or existing namespace.
CREATE TABLE collaboration_artifact_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL,
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL
);
CREATE TABLE collaboration_artifacts (
    artifact_id UUID PRIMARY KEY,
    owner_id UUID NOT NULL,
    head_revision_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991)
);
CREATE TABLE collaboration_artifact_revisions (
    revision_id UUID PRIMARY KEY,
    artifact_id UUID NOT NULL REFERENCES collaboration_artifacts(artifact_id),
    sequence BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 16384),
    UNIQUE (artifact_id, sequence)
);
CREATE TABLE collaboration_artifact_outbox (
    event_id UUID PRIMARY KEY,
    artifact_id UUID NOT NULL REFERENCES collaboration_artifacts(artifact_id),
    revision_id UUID NOT NULL REFERENCES collaboration_artifact_revisions(revision_id),
    sequence BIGINT NOT NULL CHECK (sequence BETWEEN 1 AND 9007199254740991),
    actor_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('cyanrex.artifact.created', 'cyanrex.artifact.revised')),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 16384),
    UNIQUE (artifact_id, sequence)
);
