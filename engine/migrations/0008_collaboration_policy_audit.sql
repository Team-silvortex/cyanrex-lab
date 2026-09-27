-- Explicit C1-C upgrade only. The installer adds observed baselines and changes access schema
-- version in the same transaction. Delimiters keep PL/pgSQL bodies intact during execution.
CREATE TABLE collaboration_policy_audit (
    sequence BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY
        CHECK (sequence BETWEEN 1 AND 9007199254740991),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    kind TEXT NOT NULL CHECK (kind IN ('baseline', 'changed', 'unchanged')),
    command_id UUID CHECK (command_id <> '00000000-0000-0000-0000-000000000000'),
    actor_principal_id UUID,
    request_digest TEXT CHECK (request_digest COLLATE "C" ~ '^[0-9a-f]{64}$'),
    before_state JSONB,
    after_state JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (authority_id, command_id),
    FOREIGN KEY (authority_id, workspace_id, principal_id)
        REFERENCES collaboration_memberships(authority_id, workspace_id, principal_id),
    FOREIGN KEY (authority_id, actor_principal_id)
        REFERENCES collaboration_principals(authority_id, principal_id),
    CHECK (
        (kind = 'baseline' AND command_id IS NULL AND actor_principal_id IS NULL
            AND request_digest IS NULL AND before_state IS NULL)
        OR (kind IN ('changed', 'unchanged') AND command_id IS NOT NULL
            AND actor_principal_id IS NOT NULL AND request_digest IS NOT NULL)
    )
);
-- cyanrex-statement
CREATE UNIQUE INDEX collaboration_policy_audit_revision
    ON collaboration_policy_audit(authority_id, workspace_id, principal_id, revision)
    WHERE kind IN ('baseline', 'changed');
-- cyanrex-statement
CREATE INDEX collaboration_policy_audit_history
    ON collaboration_policy_audit(authority_id, workspace_id, principal_id, sequence);
-- cyanrex-statement
CREATE FUNCTION collaboration_reject_policy_audit_mutation() RETURNS trigger
    LANGUAGE plpgsql AS $$ BEGIN
        RAISE EXCEPTION 'policy audit is append-only';
    END $$;
-- cyanrex-statement
CREATE TRIGGER collaboration_policy_audit_immutable
    BEFORE UPDATE OR DELETE ON collaboration_policy_audit
    FOR EACH ROW EXECUTE FUNCTION collaboration_reject_policy_audit_mutation();
-- cyanrex-statement
CREATE TRIGGER collaboration_policy_audit_no_truncate
    BEFORE TRUNCATE ON collaboration_policy_audit
    FOR EACH STATEMENT EXECUTE FUNCTION collaboration_reject_policy_audit_mutation();
