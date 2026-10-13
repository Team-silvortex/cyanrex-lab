#![cfg(unix)]
//! Explicit trusted-host local saving, not UI delivery, remote authority or a resumable export.
use cyanrex_engine::{
    models::collaboration::*,
    services::file_transfer::{
        staging::{AssembledFile, ChunkStaging, ChunkStagingError, FileSaveError, FileSaveTarget},
        verify_file, FILE_TRANSFER_BUFFER_BYTES,
    },
};
use uuid::Uuid;

#[path = "file_save/baseline.rs"]
mod baseline;
#[path = "file_save/binding.rs"]
mod binding;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_save/fixture.rs"]
mod fixture;
#[path = "file_save/reuse.rs"]
mod reuse;
#[path = "file_save/selection.rs"]
mod selection;
#[path = "file_save/storage.rs"]
mod storage;
#[path = "file_save/support.rs"]
mod support;
