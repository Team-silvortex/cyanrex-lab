use cyanrex_engine::{models::collaboration::*, services::file_transfer::*};
use sha2::{Digest, Sha256};

#[cfg(unix)]
#[path = "file_transfer/files.rs"]
mod files;
#[path = "file_transfer/fixture.rs"]
mod fixture;
#[path = "file_transfer/manifest.rs"]
mod manifest;
#[path = "file_transfer/streams.rs"]
mod streams;

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap(),
        workspace_id: "267c6b0f-182c-4980-a925-cdc967327c57".parse().unwrap(),
    }
}

#[test]
fn manifest_constructs_exact_one_chunk_without_publishing() {
    let sha256: Sha256Digest = format!("{:x}", Sha256::digest(b"inert bytes"))
        .parse()
        .unwrap();
    let manifest = FileTransferManifest::new(
        scope(),
        FileId::try_from(uuid::Uuid::from_u128(1)).unwrap(),
        FileVersionId::try_from(uuid::Uuid::from_u128(2)).unwrap(),
        "data.bin",
        11,
        vec![sha256.clone()],
        sha256,
    )
    .expect("valid bounded manifest should construct");
    assert_eq!(manifest.chunk(0).unwrap().byte_length, 11);
    assert_eq!(manifest.chunk_size(), FILE_TRANSFER_CHUNK_BYTES);
}

#[test]
fn streaming_inspection_and_verification_accept_real_bytes() {
    let mut input = &b"inert bytes"[..];
    let manifest = inspect_stream(
        scope(),
        FileId::try_from(uuid::Uuid::from_u128(1)).unwrap(),
        FileVersionId::try_from(uuid::Uuid::from_u128(2)).unwrap(),
        "data.bin",
        &mut input,
    )
    .expect("stream inspection should hash real bytes");
    assert_eq!(manifest.byte_length(), 11);
    verify_chunk(&manifest, 0, &mut &b"inert bytes"[..]).unwrap();
    verify_stream(&manifest, &mut &b"inert bytes"[..]).unwrap();
}
