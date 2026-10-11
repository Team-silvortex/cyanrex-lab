//! Compact typed observations only, not standalone verification of foreign manifests or receipts.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChunkRow {
    pub(super) coordinate: ChunkCoordinate,
    pub(super) nonce: Uuid,
}

/// Minted only after an acknowledged Unknown COMMIT. Metadata lookup cannot restore it.
pub(crate) struct UnknownChunk {
    pub(super) schema: JournalSchemaPin,
    pub(super) expected: ChunkRow,
}

impl ChunkRow {
    pub(super) fn fingerprint(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"cyanrex.file-upload.chunk-record.v1\0");
        let coordinate = &self.coordinate;
        let record = &coordinate.record;
        hash.update(record.upload_id.as_uuid().as_bytes());
        hash.update(record.index.to_be_bytes());
        hash.update(record.owner.authority_id.as_uuid().as_bytes());
        hash.update(record.owner.principal_id.as_uuid().as_bytes());
        hash.update(record.account_id.as_uuid().as_bytes());
        hash.update(coordinate.source_key);
        hash.update(coordinate.root_key);
        hash.update(coordinate.total_length.to_be_bytes());
        hash.update(record.byte_length.to_be_bytes());
        hash.update(record.sha256.as_str().as_bytes());
        hash.update(self.nonce.as_bytes());
        hash.update([u8::from(record.state == PersistentChunkState::Confirmed)]);
        hash.finalize().into()
    }

    pub(super) fn decode(row: &sqlx::postgres::PgRow, scope: WorkspaceRef) -> Result<Self> {
        let invalid = || FileUploadChunkJournalError::InvalidRecord;
        let upload_id =
            FileUploadId::try_from(row.try_get::<Uuid, _>("upload_id")?).map_err(|_| invalid())?;
        let principal_id =
            PrincipalId::try_from(row.try_get::<Uuid, _>("owner_id")?).map_err(|_| invalid())?;
        let account_id = LegacyAccountId::try_from(row.try_get::<Uuid, _>("account_id")?)
            .map_err(|_| invalid())?;
        let index = u32::try_from(row.try_get::<i32, _>("chunk_index")?).map_err(|_| invalid())?;
        let total_length =
            u64::try_from(row.try_get::<i64, _>("total_length")?).map_err(|_| invalid())?;
        let byte_length =
            u64::try_from(row.try_get::<i64, _>("chunk_length")?).map_err(|_| invalid())?;
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
        let sha256 = row
            .try_get::<Option<String>, _>("sha256")?
            .ok_or_else(invalid)?
            .parse::<Sha256Digest>()
            .map_err(|_| invalid())?;
        let nonce: Uuid = row.try_get("nonce")?;
        let offset = u64::from(index)
            .checked_mul(FILE_TRANSFER_CHUNK_BYTES)
            .ok_or_else(invalid)?;
        if index as usize >= MAX_FILE_TRANSFER_CHUNKS
            || total_length == 0
            || total_length > MAX_FILE_TRANSFER_BYTES
            || offset >= total_length
            || byte_length != FILE_TRANSFER_CHUNK_BYTES.min(total_length - offset)
            || nonce.get_version() != Some(uuid::Version::Random)
            || nonce.get_variant() != uuid::Variant::RFC4122
        {
            return Err(invalid());
        }
        let state = if row.try_get::<bool, _>("confirmed")? {
            PersistentChunkState::Confirmed
        } else {
            PersistentChunkState::Unknown
        };
        Ok(Self {
            coordinate: ChunkCoordinate {
                scope,
                record: UploadChunkRecord {
                    upload_id,
                    owner: PrincipalRef {
                        authority_id: scope.authority_id,
                        principal_id,
                    },
                    account_id,
                    index,
                    byte_length,
                    sha256,
                    state,
                },
                source_key,
                root_key,
                total_length,
            },
            nonce,
        })
    }
}

// CASE rejects oversized corrupt fields before copying BYTEA/TEXT into the SQL driver.
pub(super) const PROJECTION: &str = "upload_id, chunk_index, owner_id, account_id,
    CASE WHEN octet_length(source_key) = 32 THEN source_key ELSE NULL END AS source_key,
    CASE WHEN octet_length(root_key) = 32 THEN root_key ELSE NULL END AS root_key,
    total_length, chunk_length,
    CASE WHEN octet_length(sha256) = 64 THEN sha256 ELSE NULL END AS sha256, nonce, confirmed";

impl FileUploadChunkJournal {
    pub(super) async fn read_row(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        index: u32,
        owner: Option<(PrincipalRef, LegacyAccountId)>,
    ) -> Result<Option<ChunkRow>> {
        let query = format!(
            "SELECT {PROJECTION} FROM collaboration_file_upload_chunks
            WHERE upload_id=$1 AND chunk_index=$2
            AND ($3::uuid IS NULL OR owner_id=$3) AND ($4::uuid IS NULL OR account_id=$4)"
        );
        let row = sqlx::query(&query)
            .bind(upload_id.as_uuid())
            .bind(index as i32)
            .bind(owner.map(|value| value.0.principal_id.as_uuid()))
            .bind(owner.map(|value| value.1.as_uuid()))
            .fetch_optional(&mut **tx)
            .await?;
        row.map(|row| ChunkRow::decode(&row, self.scope))
            .transpose()
    }
}
