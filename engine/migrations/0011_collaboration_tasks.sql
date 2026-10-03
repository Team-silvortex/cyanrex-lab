-- Explicit C2-B task staging only. Do not add to Engine startup or apply to an existing namespace.
CREATE TABLE collaboration_task_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL,
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL
);
CREATE TABLE collaboration_tasks (
    task_id UUID PRIMARY KEY,
    owner_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 32768)
);
CREATE TABLE collaboration_task_outbox (
    event_id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES collaboration_tasks(task_id),
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    actor_id UUID NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('cyanrex.task.created', 'cyanrex.task.status_changed')),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 32768),
    UNIQUE (task_id, revision)
);
