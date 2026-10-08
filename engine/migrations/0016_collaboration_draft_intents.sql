CREATE TABLE collaboration_draft_intent_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version > 0),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    installation_id UUID NOT NULL
);
CREATE TABLE collaboration_draft_intents (
    task_id UUID PRIMARY KEY,
    owner_id UUID NOT NULL,
    account_id UUID NOT NULL,
    source_namespace TEXT NOT NULL,
    task_namespace TEXT NOT NULL,
    artifact_namespace TEXT NOT NULL,
    checkpoint BYTEA NOT NULL CHECK (octet_length(checkpoint) <= 65536)
);
