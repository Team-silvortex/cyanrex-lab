-- Explicit C1-B staging installation only, after the independent identity schema.
-- Never run by Engine startup or authentication. No legacy users/sessions are changed.
CREATE TABLE IF NOT EXISTS collaboration_access_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version > 0)
);

CREATE TABLE collaboration_memberships (
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    role_refs TEXT[] NOT NULL CHECK (
        cardinality(role_refs) <= 2
        AND array_position(role_refs, NULL) IS NULL
        AND role_refs <@ ARRAY['cyanrex.workspace.owner', 'cyanrex.teaching.teacher', 'cyanrex.teaching.learner']::TEXT[]
    ),
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended')),
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (authority_id, workspace_id, principal_id),
    FOREIGN KEY (authority_id, workspace_id)
        REFERENCES collaboration_workspaces(authority_id, workspace_id),
    FOREIGN KEY (authority_id, principal_id)
        REFERENCES collaboration_principals(authority_id, principal_id)
);

-- A revoked row is retained, not deleted: stale create/replace cannot bypass the revision fence.
-- The source workspace records the original teaching migration, not the grant's effective scope.
CREATE TABLE collaboration_deployment_grants (
    authority_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    source_workspace_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (authority_id, principal_id),
    FOREIGN KEY (authority_id, source_workspace_id, principal_id)
        REFERENCES collaboration_memberships(authority_id, workspace_id, principal_id)
);

INSERT INTO collaboration_access_schema (singleton, version) VALUES (TRUE, 1);
