//! One bounded canonical manifest per row, never an aggregate of all publication bodies.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PublishedRow {
    pub(super) coordinate: PublicationCoordinate,
    pub(super) nonce: Uuid,
}

pub(super) fn canonical_manifest(manifest: &FileTransferManifest) -> Result<Vec<u8>> {
    let raw =
        serde_json::to_vec(manifest).map_err(|_| FileUploadPublicationStoreError::InvalidRecord)?;
    if raw.len() > MAX_FILE_TRANSFER_MANIFEST_BYTES {
        return Err(FileUploadPublicationStoreError::InvalidRecord);
    }
    Ok(raw)
}

pub(super) fn fingerprint(row: &PublishedRow, raw: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"cyanrex.file-upload.publication-record.v1\0");
    let coordinate = &row.coordinate;
    let record = &coordinate.record;
    hash.update(record.upload_id.as_uuid().as_bytes());
    hash.update(record.owner.authority_id.as_uuid().as_bytes());
    hash.update(record.owner.principal_id.as_uuid().as_bytes());
    hash.update(record.account_id.as_uuid().as_bytes());
    hash.update(record.manifest.file_id().as_uuid().as_bytes());
    hash.update(record.manifest.version_id().as_uuid().as_bytes());
    hash.update(coordinate.source_key);
    hash.update(coordinate.root_key);
    hash.update(coordinate.journal_key);
    hash.update(row.nonce.as_bytes());
    hash.update((raw.len() as u32).to_be_bytes());
    hash.update(raw);
    hash.finalize().into()
}

impl FileUploadPublicationStore {
    pub(super) async fn read_record(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: Option<(PrincipalRef, LegacyAccountId)>,
    ) -> Result<Option<(PublishedRow, [u8; 32])>> {
        self.read_record_with_lock(tx, upload_id, owner, false)
            .await
    }

    pub(super) async fn read_record_with_lock(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: Option<(PrincipalRef, LegacyAccountId)>,
        share: bool,
    ) -> Result<Option<(PublishedRow, [u8; 32])>> {
        // Reject oversized corrupt bodies and fixed keys before driver allocation/decoding.
        let query = if share {
            "SELECT upload_id,file_id,version_id,owner_id,account_id,nonce,
            CASE WHEN octet_length(manifest) <= $4 THEN manifest ELSE NULL END AS manifest,
            CASE WHEN octet_length(source_key) = 32 THEN source_key ELSE NULL END AS source_key,
            CASE WHEN octet_length(root_key) = 32 THEN root_key ELSE NULL END AS root_key,
            CASE WHEN octet_length(journal_key) = 32 THEN journal_key ELSE NULL END AS journal_key
            FROM collaboration_file_publications WHERE upload_id=$1
            AND ($2::uuid IS NULL OR owner_id=$2) AND ($3::uuid IS NULL OR account_id=$3)
            FOR SHARE"
        } else {
            "SELECT upload_id,file_id,version_id,owner_id,account_id,nonce,
            CASE WHEN octet_length(manifest) <= $4 THEN manifest ELSE NULL END AS manifest,
            CASE WHEN octet_length(source_key) = 32 THEN source_key ELSE NULL END AS source_key,
            CASE WHEN octet_length(root_key) = 32 THEN root_key ELSE NULL END AS root_key,
            CASE WHEN octet_length(journal_key) = 32 THEN journal_key ELSE NULL END AS journal_key
            FROM collaboration_file_publications WHERE upload_id=$1
            AND ($2::uuid IS NULL OR owner_id=$2) AND ($3::uuid IS NULL OR account_id=$3)"
        };
        let row = sqlx::query(query)
            .bind(upload_id.as_uuid())
            .bind(owner.map(|value| value.0.principal_id.as_uuid()))
            .bind(owner.map(|value| value.1.as_uuid()))
            .bind(MAX_FILE_TRANSFER_MANIFEST_BYTES as i32)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let invalid = || FileUploadPublicationStoreError::InvalidRecord;
        let selected_id =
            FileUploadId::try_from(row.try_get::<Uuid, _>("upload_id")?).map_err(|_| invalid())?;
        let file_id =
            FileId::try_from(row.try_get::<Uuid, _>("file_id")?).map_err(|_| invalid())?;
        let version_id = FileVersionId::try_from(row.try_get::<Uuid, _>("version_id")?)
            .map_err(|_| invalid())?;
        let principal_id =
            PrincipalId::try_from(row.try_get::<Uuid, _>("owner_id")?).map_err(|_| invalid())?;
        let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)
            .map_err(|_| invalid())?;
        let source_key = row
            .try_get::<Option<Vec<u8>>, _>("source_key")?
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        let root_key = row
            .try_get::<Option<Vec<u8>>, _>("root_key")?
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        let journal_key = row
            .try_get::<Option<Vec<u8>>, _>("journal_key")?
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        let nonce: Uuid = row.try_get("nonce")?;
        let raw = row
            .try_get::<Option<Vec<u8>>, _>("manifest")?
            .ok_or_else(invalid)?;
        let manifest = FileTransferManifest::parse_json(&raw).map_err(|_| invalid())?;
        if selected_id != upload_id
            || manifest.workspace() != self.scope
            || manifest.file_id() != file_id
            || manifest.version_id() != version_id
            || canonical_manifest(&manifest)? != raw
            || nonce.get_version() != Some(uuid::Version::Random)
            || nonce.get_variant() != uuid::Variant::RFC4122
        {
            return Err(invalid());
        }
        let record = FileUploadPublicationRecord {
            upload_id: selected_id,
            owner: PrincipalRef {
                authority_id: self.scope.authority_id,
                principal_id,
            },
            account_id,
            manifest,
        };
        if owner.is_some_and(|owner| owner != (record.owner, record.account_id)) {
            return Err(invalid());
        }
        let row = PublishedRow {
            coordinate: PublicationCoordinate {
                record,
                source_key,
                root_key,
                journal_key,
            },
            nonce,
        };
        let digest = fingerprint(&row, &raw);
        Ok(Some((row, digest)))
    }
}
