CREATE TABLE collaboration_draft_dispatch_steps (
    task_id UUID NOT NULL REFERENCES collaboration_draft_intents(task_id),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0 AND ordinal <= 32),
    nonce UUID NOT NULL UNIQUE,
    committed BOOLEAN NOT NULL,
    PRIMARY KEY (task_id, ordinal)
);
