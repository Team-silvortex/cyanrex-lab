CREATE TABLE collaboration_file_publication_schema (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    version INTEGER NOT NULL CHECK (version = 1),
    authority_id UUID NOT NULL,
    workspace_id UUID NOT NULL,
    installation_id UUID NOT NULL,
    namespace_oid BIGINT NOT NULL CHECK (namespace_oid > 0),
    schema_oid BIGINT NOT NULL CHECK (schema_oid > 0),
    publications_oid BIGINT NOT NULL CHECK (publications_oid > 0),
    max_publications INTEGER NOT NULL CHECK (max_publications BETWEEN 1 AND 256)
);
CREATE TABLE collaboration_file_publications (
    upload_id UUID PRIMARY KEY,
    file_id UUID NOT NULL,
    version_id UUID NOT NULL,
    owner_id UUID NOT NULL,
    account_id UUID NOT NULL,
    manifest BYTEA NOT NULL CHECK (octet_length(manifest) <= 1179648),
    source_key BYTEA NOT NULL CHECK (octet_length(source_key) = 32),
    root_key BYTEA NOT NULL CHECK (octet_length(root_key) = 32),
    journal_key BYTEA NOT NULL CHECK (octet_length(journal_key) = 32),
    nonce UUID NOT NULL UNIQUE CHECK (
        substring(nonce::text FROM 15 FOR 1) = '4' AND
        substring(nonce::text FROM 20 FOR 1) IN ('8', '9', 'a', 'b')
    ),
    UNIQUE (file_id, version_id)
);
