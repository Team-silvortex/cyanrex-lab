-- Explicit C1 identity-store installation only. Not executed by Engine startup.
CREATE TABLE IF NOT EXISTS collaboration_identity_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version > 0)
);

CREATE TABLE collaboration_authorities (
    authority_id UUID PRIMARY KEY CHECK (authority_id <> '00000000-0000-0000-0000-000000000000'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE collaboration_workspaces (
    authority_id UUID NOT NULL REFERENCES collaboration_authorities(authority_id),
    workspace_id UUID NOT NULL CHECK (workspace_id <> '00000000-0000-0000-0000-000000000000'),
    title TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    PRIMARY KEY (authority_id, workspace_id)
);

CREATE TABLE collaboration_legacy_workspaces (
    authority_id UUID PRIMARY KEY REFERENCES collaboration_authorities(authority_id),
    workspace_id UUID NOT NULL,
    FOREIGN KEY (authority_id, workspace_id)
        REFERENCES collaboration_workspaces(authority_id, workspace_id)
);

CREATE TABLE collaboration_principals (
    authority_id UUID NOT NULL REFERENCES collaboration_authorities(authority_id),
    principal_id UUID NOT NULL CHECK (principal_id <> '00000000-0000-0000-0000-000000000000'),
    kind TEXT NOT NULL CHECK (kind IN ('human', 'agent', 'service', 'machine')),
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (authority_id, principal_id)
);

CREATE TABLE collaboration_legacy_identities (
    authority_id UUID NOT NULL,
    username TEXT NOT NULL CHECK (
        octet_length(username) BETWEEN 3 AND 64
        AND username COLLATE "C" ~ '^[a-z0-9_.-]+$'
    ),
    account_id UUID NOT NULL CHECK (account_id <> '00000000-0000-0000-0000-000000000000'),
    principal_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retired_at TIMESTAMPTZ,
    PRIMARY KEY (authority_id, username, account_id),
    UNIQUE (authority_id, account_id),
    UNIQUE (authority_id, principal_id),
    FOREIGN KEY (authority_id, principal_id)
        REFERENCES collaboration_principals(authority_id, principal_id)
);

CREATE UNIQUE INDEX collaboration_one_active_legacy_username
    ON collaboration_legacy_identities(authority_id, username) WHERE retired_at IS NULL;

INSERT INTO collaboration_identity_schema (singleton, version) VALUES (TRUE, 1);
