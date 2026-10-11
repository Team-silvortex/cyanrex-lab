//! Inert caller-supplied file permission snapshots, not authenticated or current authority.
//!
//! Matching neither proves provenance, freshness, publication, available bytes nor delivery.
//! These contracts allocate no identities, consult no roles and perform no storage or file IO.
//! A copied Active snapshot does not read current revocation, and no trusted issuer is established.
//! Grant identities are distinct from upload identities, despite using UUID strings in JSON:
//!
//! ```compile_fail
//! use cyanrex_engine::models::collaboration::{FileGrantId, FileUploadId};
//! let grant: FileGrantId = "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap();
//! let upload: FileUploadId = grant;
//! ```
use super::{
    file_transfer::MapOnly, ContractError, CoreSchemaVersion, FileGrantId, FileId,
    FileTransferManifest, FileVersionId, LegacyAccountId, PrincipalRef, RevisionNumber,
    Sha256Digest, WorkspaceRef, MAX_FILE_TRANSFER_BYTES,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const MAX_FILE_GRANT_JSON_BYTES: usize = 4096;
pub const MAX_FILE_GRANT_UNIX_SECONDS: u64 = 253_402_300_799;
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Exact Principal and legacy account incarnation, not a credential or a role conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFileAccountRef>")]
pub struct FileAccountRef {
    principal: PrincipalRef,
    account_id: LegacyAccountId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFileAccountRef {
    principal: MapOnly<PrincipalRef>,
    account_id: LegacyAccountId,
}

impl TryFrom<MapOnly<RawFileAccountRef>> for FileAccountRef {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFileAccountRef>) -> Result<Self, Self::Error> {
        let value = value.0;
        Ok(Self::new(value.principal.0, value.account_id))
    }
}

impl FileAccountRef {
    pub fn new(principal: PrincipalRef, account_id: LegacyAccountId) -> Self {
        Self {
            principal,
            account_id,
        }
    }

    pub fn principal(&self) -> PrincipalRef {
        self.principal
    }

    pub fn account_id(&self) -> LegacyAccountId {
        self.account_id
    }
}

/// Exact claimed file version and bytes, not a stored-file proof or a download grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFileVersionRef>")]
pub struct FileVersionRef {
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    byte_length: u64,
    sha256: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFileVersionRef {
    workspace: MapOnly<WorkspaceRef>,
    file_id: FileId,
    version_id: FileVersionId,
    byte_length: u64,
    sha256: Sha256Digest,
}

impl TryFrom<MapOnly<RawFileVersionRef>> for FileVersionRef {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFileVersionRef>) -> Result<Self, Self::Error> {
        let value = value.0;
        if value.byte_length > MAX_FILE_TRANSFER_BYTES
            || (value.byte_length == 0 && value.sha256.as_str() != EMPTY_SHA256)
        {
            return Err(ContractError("invalid file version reference"));
        }
        Ok(Self {
            workspace: value.workspace.0,
            file_id: value.file_id,
            version_id: value.version_id,
            byte_length: value.byte_length,
            sha256: value.sha256,
        })
    }
}

impl FileVersionRef {
    pub fn from_manifest(manifest: &FileTransferManifest) -> Self {
        Self {
            workspace: manifest.workspace(),
            file_id: manifest.file_id(),
            version_id: manifest.version_id(),
            byte_length: manifest.byte_length(),
            sha256: manifest.sha256().clone(),
        }
    }

    pub fn workspace(&self) -> WorkspaceRef {
        self.workspace
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn version_id(&self) -> FileVersionId {
        self.version_id
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}

/// Mandatory bounded UTC Unix seconds. Validity is half-open: [not_before, expires_at).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFileGrantValidity>")]
pub struct FileGrantValidity {
    not_before: u64,
    expires_at: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFileGrantValidity {
    not_before: u64,
    expires_at: u64,
}

impl TryFrom<MapOnly<RawFileGrantValidity>> for FileGrantValidity {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFileGrantValidity>) -> Result<Self, Self::Error> {
        Self::new(value.0.not_before, value.0.expires_at)
    }
}

impl FileGrantValidity {
    pub fn new(not_before: u64, expires_at: u64) -> Result<Self, ContractError> {
        if not_before >= expires_at || expires_at > MAX_FILE_GRANT_UNIX_SECONDS {
            return Err(ContractError("invalid file grant validity"));
        }
        Ok(Self {
            not_before,
            expires_at,
        })
    }

    pub fn not_before(&self) -> u64 {
        self.not_before
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn contains(&self, now: u64) -> bool {
        now <= MAX_FILE_GRANT_UNIX_SECONDS && now >= self.not_before && now < self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum FileGrantState {
    Active,
    Revoked,
}

impl TryFrom<String> for FileGrantState {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "active" => Ok(Self::Active),
            "revoked" => Ok(Self::Revoked),
            _ => Err(ContractError("invalid file grant state")),
        }
    }
}

/// Workspace publisher permission metadata. Matching supplied snapshots is not authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFilePublisherGrant>")]
pub struct FilePublisherGrant {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    workspace: WorkspaceRef,
    subject: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFilePublisherGrant {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    workspace: MapOnly<WorkspaceRef>,
    subject: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}

impl TryFrom<MapOnly<RawFilePublisherGrant>> for FilePublisherGrant {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFilePublisherGrant>) -> Result<Self, Self::Error> {
        let value = value.0;
        let CoreSchemaVersion = value.schema_version;
        Self::new(
            value.id,
            value.revision,
            value.workspace.0,
            value.subject,
            value.validity,
            value.state,
        )
    }
}

impl FilePublisherGrant {
    pub fn new(
        id: FileGrantId,
        revision: RevisionNumber,
        workspace: WorkspaceRef,
        subject: FileAccountRef,
        validity: FileGrantValidity,
        state: FileGrantState,
    ) -> Result<Self, ContractError> {
        if subject.principal.authority_id != workspace.authority_id {
            return Err(ContractError(
                "file publisher grant authority does not match",
            ));
        }
        Ok(Self {
            schema_version: CoreSchemaVersion,
            id,
            revision,
            workspace,
            subject,
            validity,
            state,
        })
    }

    /// Strict bounded metadata JSON; other Serde callers must enforce their own byte limit.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        parse_grant(bytes)
    }

    pub fn schema_version(&self) -> CoreSchemaVersion {
        self.schema_version
    }
    pub fn id(&self) -> FileGrantId {
        self.id
    }
    pub fn revision(&self) -> RevisionNumber {
        self.revision
    }
    pub fn workspace(&self) -> WorkspaceRef {
        self.workspace
    }
    pub fn subject(&self) -> FileAccountRef {
        self.subject
    }
    pub fn validity(&self) -> FileGrantValidity {
        self.validity
    }
    pub fn state(&self) -> FileGrantState {
        self.state
    }

    /// Compare only these supplied snapshots. False confers no authority; true establishes neither
    /// a trusted issuer nor current permission. A copied Active record does not read revocation.
    pub fn matches(&self, scope: WorkspaceRef, subject: FileAccountRef, now: u64) -> bool {
        self.state == FileGrantState::Active
            && self.workspace == scope
            && self.subject == subject
            && self.validity.contains(now)
    }
}

/// Independently active exact-file recipient metadata. It is not linked to a publisher-grant
/// ID/revision: revoking publisher permission does not implicitly revoke this recipient record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFileRecipientGrant>")]
pub struct FileRecipientGrant {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    file: FileVersionRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFileRecipientGrant {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    file: FileVersionRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}

impl TryFrom<MapOnly<RawFileRecipientGrant>> for FileRecipientGrant {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFileRecipientGrant>) -> Result<Self, Self::Error> {
        let value = value.0;
        let CoreSchemaVersion = value.schema_version;
        Self::new(
            value.id,
            value.revision,
            value.file,
            value.publisher,
            value.recipient,
            value.validity,
            value.state,
        )
    }
}

impl FileRecipientGrant {
    pub fn new(
        id: FileGrantId,
        revision: RevisionNumber,
        file: FileVersionRef,
        publisher: FileAccountRef,
        recipient: FileAccountRef,
        validity: FileGrantValidity,
        state: FileGrantState,
    ) -> Result<Self, ContractError> {
        let authority = file.workspace.authority_id;
        if publisher.principal.authority_id != authority
            || recipient.principal.authority_id != authority
        {
            return Err(ContractError(
                "file recipient grant authority does not match",
            ));
        }
        Ok(Self {
            schema_version: CoreSchemaVersion,
            id,
            revision,
            file,
            publisher,
            recipient,
            validity,
            state,
        })
    }

    /// Strict bounded metadata JSON; other Serde callers must enforce their own byte limit.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        parse_grant(bytes)
    }

    pub fn schema_version(&self) -> CoreSchemaVersion {
        self.schema_version
    }
    pub fn id(&self) -> FileGrantId {
        self.id
    }
    pub fn revision(&self) -> RevisionNumber {
        self.revision
    }
    pub fn file(&self) -> &FileVersionRef {
        &self.file
    }
    pub fn publisher(&self) -> FileAccountRef {
        self.publisher
    }
    pub fn recipient(&self) -> FileAccountRef {
        self.recipient
    }
    pub fn validity(&self) -> FileGrantValidity {
        self.validity
    }
    pub fn state(&self) -> FileGrantState {
        self.state
    }

    /// Compare only these supplied snapshots. False confers no authority; true establishes neither
    /// a trusted issuer nor current revocation state or delivery. Publisher-grant state is not read.
    pub fn matches(
        &self,
        file: &FileVersionRef,
        publisher: FileAccountRef,
        recipient: FileAccountRef,
        now: u64,
    ) -> bool {
        self.state == FileGrantState::Active
            && &self.file == file
            && self.publisher == publisher
            && self.recipient == recipient
            && self.validity.contains(now)
    }
}

fn parse_grant<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ContractError> {
    if bytes.len() > MAX_FILE_GRANT_JSON_BYTES {
        return Err(ContractError("file grant exceeds its byte limit"));
    }
    serde_json::from_slice(bytes).map_err(|_| ContractError("invalid file grant"))
}
