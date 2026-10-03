-- Task storage schema 2, fresh staging only. Never apply to an existing namespace or at startup.
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
    event_type TEXT NOT NULL CHECK (event_type IN ('cyanrex.task.created', 'cyanrex.task.status_changed', 'cyanrex.task.inputs_replaced')),
    snapshot TEXT NOT NULL CHECK (octet_length(snapshot) <= 32768),
    UNIQUE (task_id, revision)
);
