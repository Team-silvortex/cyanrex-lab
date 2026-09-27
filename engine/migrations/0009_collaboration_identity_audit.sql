-- Explicit C1-D staging upgrade only. The installer appends observed baselines and updates
-- identity schema 1 -> 2 in the same transaction. Never executed at Engine startup.
CREATE TABLE collaboration_identity_audit (
    sequence BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY
        CHECK (sequence BETWEEN 1 AND 9007199254740991),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    username TEXT NOT NULL,
    account_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('baseline', 'bound', 'retired', 'unchanged')),
    operation TEXT CHECK (operation IN ('bind', 'retire')),
    command_id UUID CHECK (command_id <> '00000000-0000-0000-0000-000000000000'),
    actor_principal_id UUID,
    request_digest TEXT CHECK (request_digest COLLATE "C" ~ '^[0-9a-f]{64}$'),
    before_state JSONB,
    after_state JSONB NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (authority_id, command_id),
    FOREIGN KEY (authority_id, workspace_id)
        REFERENCES collaboration_workspaces(authority_id, workspace_id),
    FOREIGN KEY (authority_id, principal_id)
        REFERENCES collaboration_principals(authority_id, principal_id),
    FOREIGN KEY (authority_id, username, account_id)
        REFERENCES collaboration_legacy_identities(authority_id, username, account_id),
    FOREIGN KEY (authority_id, actor_principal_id)
        REFERENCES collaboration_principals(authority_id, principal_id),
    CHECK (
        (kind = 'baseline' AND operation IS NULL AND command_id IS NULL AND actor_principal_id IS NULL
            AND request_digest IS NULL AND before_state IS NULL)
        OR (kind <> 'baseline' AND operation IS NOT NULL AND command_id IS NOT NULL
            AND actor_principal_id IS NOT NULL AND request_digest IS NOT NULL)
    )
);
-- cyanrex-statement
CREATE UNIQUE INDEX collaboration_identity_audit_transitions
    ON collaboration_identity_audit(authority_id, principal_id, kind)
    WHERE kind IN ('baseline', 'bound', 'retired');
-- cyanrex-statement
CREATE INDEX collaboration_identity_audit_history
    ON collaboration_identity_audit(authority_id, workspace_id, principal_id, sequence);
-- cyanrex-statement
CREATE INDEX collaboration_identity_audit_incarnation
    ON collaboration_identity_audit(authority_id, workspace_id, username, account_id, sequence);
-- cyanrex-statement
CREATE FUNCTION collaboration_reject_identity_audit_mutation() RETURNS trigger
    LANGUAGE plpgsql AS $$ BEGIN
        RAISE EXCEPTION 'identity audit is append-only';
    END $$;
-- cyanrex-statement
CREATE TRIGGER collaboration_identity_audit_immutable
    BEFORE UPDATE OR DELETE ON collaboration_identity_audit
    FOR EACH ROW EXECUTE FUNCTION collaboration_reject_identity_audit_mutation();
-- cyanrex-statement
CREATE TRIGGER collaboration_identity_audit_no_truncate
    BEFORE TRUNCATE ON collaboration_identity_audit
    FOR EACH STATEMENT EXECUTE FUNCTION collaboration_reject_identity_audit_mutation();
