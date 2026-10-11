//! Transport-neutral file metadata. Parsing does not authorize a transfer or prove stored bytes.
use super::{ContractError, CoreSchemaVersion, FileId, FileVersionId, Sha256Digest, WorkspaceRef};
use serde::{
    de::{value::MapAccessDeserializer, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::{fmt, marker::PhantomData};

pub const FILE_TRANSFER_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_FILE_TRANSFER_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_FILE_TRANSFER_CHUNKS: usize = 16_384;
pub const MAX_FILE_TRANSFER_MANIFEST_BYTES: usize = 1152 * 1024;

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

// Forward maps without collecting into Value, preserving duplicate fields for strict derives.
pub(super) struct MapOnly<T>(pub(super) T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RecordVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordVisitor<T> {
            type Value = MapOnly<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object record")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }

        deserializer.deserialize_map(RecordVisitor(PhantomData))
    }
}

struct BoundedChunkDigests(Vec<Sha256Digest>);

impl<'de> Deserialize<'de> for BoundedChunkDigests {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ChunkVisitor;
        struct RejectExtra;

        impl<'de> DeserializeSeed<'de> for RejectExtra {
            type Value = ();

            fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<Self::Value, D::Error> {
                Err(serde::de::Error::custom(
                    "file transfer manifest has too many chunks",
                ))
            }
        }

        impl<'de> Visitor<'de> for ChunkVisitor {
            type Value = BoundedChunkDigests;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded chunk digest sequence")
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                // The producer's size hint is untrusted; grow only for successfully decoded items.
                let mut chunks = Vec::new();
                while chunks.len() < MAX_FILE_TRANSFER_CHUNKS {
                    match sequence.next_element::<Sha256Digest>()? {
                        Some(chunk) => chunks.push(chunk),
                        None => return Ok(BoundedChunkDigests(chunks)),
                    }
                }
                // An extra value fails before that value is deserialized, even if it is malformed.
                let _ = sequence.next_element_seed(RejectExtra)?;
                Ok(BoundedChunkDigests(chunks))
            }
        }

        deserializer.deserialize_seq(ChunkVisitor)
    }
}

/// Immutable claimed metadata, not proof of bytes, ownership, availability or transfer authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawFileTransferManifest>")]
pub struct FileTransferManifest {
    schema_version: CoreSchemaVersion,
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: String,
    byte_length: u64,
    chunk_size: u64,
    chunk_sha256: Vec<Sha256Digest>,
    sha256: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFileTransferManifest {
    schema_version: CoreSchemaVersion,
    workspace: MapOnly<WorkspaceRef>,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: String,
    byte_length: u64,
    chunk_size: u64,
    chunk_sha256: BoundedChunkDigests,
    sha256: Sha256Digest,
}

impl TryFrom<MapOnly<RawFileTransferManifest>> for FileTransferManifest {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawFileTransferManifest>) -> Result<Self, Self::Error> {
        let value = value.0;
        let CoreSchemaVersion = value.schema_version;
        if value.chunk_size != FILE_TRANSFER_CHUNK_BYTES {
            return Err(ContractError("invalid file transfer manifest"));
        }
        Self::new(
            value.workspace.0,
            value.file_id,
            value.version_id,
            value.display_name,
            value.byte_length,
            value.chunk_sha256.0,
            value.sha256,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChunkSpec {
    pub index: u32,
    pub offset: u64,
    pub byte_length: u64,
    pub sha256: Sha256Digest,
}

/// Inert portable display metadata: never a path or a request to normalize a filename.
pub(crate) fn validate_file_display_name(display_name: &str) -> Result<(), ContractError> {
    if display_name.is_empty()
        || display_name.encode_utf16().take(129).count() > 128
        || display_name.trim() != display_name
        || matches!(display_name, "." | "..")
        || display_name.ends_with('.')
        || display_name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
    {
        return Err(ContractError(
            "file display name must be a bounded portable basename",
        ));
    }
    Ok(())
}

impl FileTransferManifest {
    pub fn new(
        workspace: WorkspaceRef,
        file_id: FileId,
        version_id: FileVersionId,
        display_name: impl Into<String>,
        byte_length: u64,
        chunk_sha256: Vec<Sha256Digest>,
        sha256: Sha256Digest,
    ) -> Result<Self, ContractError> {
        let display_name = display_name.into();
        validate_file_display_name(&display_name)?;
        if byte_length > MAX_FILE_TRANSFER_BYTES || chunk_sha256.len() > MAX_FILE_TRANSFER_CHUNKS {
            return Err(ContractError("invalid file transfer manifest"));
        }
        let chunk_count = byte_length / FILE_TRANSFER_CHUNK_BYTES
            + u64::from(byte_length % FILE_TRANSFER_CHUNK_BYTES != 0);
        if u64::try_from(chunk_sha256.len()).ok() != Some(chunk_count)
            || (byte_length == 0 && sha256.as_str() != EMPTY_SHA256)
            || (chunk_sha256.len() == 1 && chunk_sha256[0] != sha256)
        {
            return Err(ContractError("invalid file transfer manifest"));
        }
        Ok(Self {
            schema_version: CoreSchemaVersion,
            workspace,
            file_id,
            version_id,
            display_name,
            byte_length,
            chunk_size: FILE_TRANSFER_CHUNK_BYTES,
            chunk_sha256,
            sha256,
        })
    }

    /// Strict bounded metadata JSON. Other Serde callers must enforce their own input byte limit.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        if bytes.len() > MAX_FILE_TRANSFER_MANIFEST_BYTES {
            return Err(ContractError(
                "file transfer manifest exceeds its byte limit",
            ));
        }
        serde_json::from_slice(bytes).map_err(|_| ContractError("invalid file transfer manifest"))
    }

    pub fn schema_version(&self) -> CoreSchemaVersion {
        self.schema_version
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

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    pub fn chunk_sha256(&self) -> &[Sha256Digest] {
        &self.chunk_sha256
    }

    pub fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }

    pub fn chunk(&self, index: u32) -> Result<FileChunkSpec, ContractError> {
        let error = ContractError("invalid file transfer chunk index");
        let position = usize::try_from(index).map_err(|_| error.clone())?;
        let sha256 = self
            .chunk_sha256
            .get(position)
            .ok_or_else(|| error.clone())?;
        let offset = u64::from(index)
            .checked_mul(FILE_TRANSFER_CHUNK_BYTES)
            .ok_or_else(|| error.clone())?;
        let remaining = self
            .byte_length
            .checked_sub(offset)
            .ok_or_else(|| error.clone())?;
        let byte_length = remaining.min(FILE_TRANSFER_CHUNK_BYTES);
        if byte_length == 0 || offset.checked_add(byte_length).is_none() {
            return Err(error);
        }
        Ok(FileChunkSpec {
            index,
            offset,
            byte_length,
            sha256: sha256.clone(),
        })
    }
}
