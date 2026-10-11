use super::*;
use sha2::{Digest, Sha256};

pub(super) fn canonical_manifest(manifest: &FileTransferManifest) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| FileUploadStoreError::InvalidRecord)?;
    if bytes.len() > MAX_FILE_TRANSFER_MANIFEST_BYTES {
        return Err(FileUploadStoreError::InvalidRecord);
    }
    Ok(bytes)
}

pub(super) fn fingerprint(record: &FileUploadReservation, manifest: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"cyanrex.file-upload.record.v1\0");
    hash.update(record.upload_id.as_uuid().as_bytes());
    hash.update(record.owner.principal_id.as_uuid().as_bytes());
    hash.update(record.account_id.as_uuid().as_bytes());
    hash.update(manifest);
    hash.finalize().into()
}

impl FileUploadStore {
    pub(super) async fn read_record(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: Option<(PrincipalRef, LegacyAccountId)>,
    ) -> Result<Option<(FileUploadReservation, [u8; 32])>> {
        self.read_record_with_lock(tx, upload_id, owner, false)
            .await
    }

    pub(super) async fn read_record_with_lock(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: Option<(PrincipalRef, LegacyAccountId)>,
        share: bool,
    ) -> Result<Option<(FileUploadReservation, [u8; 32])>> {
        let query = if share {
            "SELECT upload_id, owner_id, account_id,
            CASE WHEN octet_length(manifest) <= $4 THEN manifest ELSE NULL END AS manifest
            FROM collaboration_file_uploads WHERE upload_id = $1
            AND ($2::uuid IS NULL OR owner_id = $2) AND ($3::uuid IS NULL OR account_id = $3)
            FOR SHARE"
        } else {
            "SELECT upload_id, owner_id, account_id,
            CASE WHEN octet_length(manifest) <= $4 THEN manifest ELSE NULL END AS manifest
            FROM collaboration_file_uploads WHERE upload_id = $1
            AND ($2::uuid IS NULL OR owner_id = $2) AND ($3::uuid IS NULL OR account_id = $3)"
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
        let invalid = || FileUploadStoreError::InvalidRecord;
        let selected_id =
            FileUploadId::try_from(row.try_get::<Uuid, _>("upload_id")?).map_err(|_| invalid())?;
        let principal_id =
            PrincipalId::try_from(row.try_get::<Uuid, _>("owner_id")?).map_err(|_| invalid())?;
        let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)
            .map_err(|_| invalid())?;
        let raw: Option<Vec<u8>> = row.try_get("manifest")?;
        let raw = raw.ok_or_else(invalid)?;
        let manifest = FileTransferManifest::parse_json(&raw).map_err(|_| invalid())?;
        if selected_id != upload_id
            || manifest.workspace() != self.scope
            || canonical_manifest(&manifest)? != raw
        {
            return Err(invalid());
        }
        let record = FileUploadReservation {
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
        let digest = fingerprint(&record, &raw);
        Ok(Some((record, digest)))
    }
}
