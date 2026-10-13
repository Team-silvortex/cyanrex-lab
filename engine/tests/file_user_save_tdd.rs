#![cfg(unix)]
//! Explicit ordinary host-directory targets without relaxing private staging or remote authority.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        file_chunk_client::{
            FileChunkClient, FileChunkClientConfig, FileChunkTransport, FileGrantedSaveError,
        },
        file_transfer::{
            staging::{
                AssembledFile, ChunkStaging, ChunkStagingError, FileSaveError, FileSaveTarget,
            },
            verify_chunk, verify_file, FILE_TRANSFER_BUFFER_BYTES,
        },
    },
};
use uuid::Uuid;

#[path = "file_user_save/baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_user_save/drift.rs"]
mod drift;
#[path = "file_user_save/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "file_download_client/fixture.rs"]
mod legacy_fixture;
#[path = "file_user_save/network.rs"]
mod network;
#[path = "file_user_save/network_fixture.rs"]
mod network_fixture;
#[allow(dead_code)]
#[path = "file_manifest_client/reply.rs"]
mod reply;
#[path = "file_user_save/reuse.rs"]
mod reuse;
#[path = "file_user_save/selection.rs"]
mod selection;
#[path = "file_user_save/storage.rs"]
mod storage;
#[path = "file_user_save/support.rs"]
mod support;
