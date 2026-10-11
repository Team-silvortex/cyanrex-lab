use super::*;
use crate::models::collaboration::{
    validate_file_display_name, Sha256Digest, FILE_TRANSFER_CHUNK_BYTES, MAX_FILE_TRANSFER_BYTES,
};
use sha2::{Digest, Sha256};
use std::io::{Error, ErrorKind};

fn invalid_io(message: &'static str) -> FileTransferError {
    Error::new(ErrorKind::InvalidData, message).into()
}

fn read<R: Read + ?Sized>(reader: &mut R, buffer: &mut [u8]) -> Result<usize, FileTransferError> {
    // Interrupted is returned to the caller; this synchronous core has no hidden retry deadline.
    let count = reader.read(buffer)?;
    if count > buffer.len() {
        return Err(invalid_io("reader exceeded the requested byte count"));
    }
    Ok(count)
}

fn write<W: Write + ?Sized>(writer: &mut W, mut bytes: &[u8]) -> Result<(), FileTransferError> {
    while !bytes.is_empty() {
        let count = writer.write(bytes)?;
        if count == 0 {
            return Err(Error::new(ErrorKind::WriteZero, "writer made no progress").into());
        }
        if count > bytes.len() {
            return Err(invalid_io("writer exceeded the offered byte count"));
        }
        bytes = &bytes[count..];
    }
    Ok(())
}

fn digest(hasher: Sha256) -> Result<Sha256Digest, FileTransferError> {
    format!("{:x}", hasher.finalize())
        .parse()
        .map_err(|_| FileTransferError::InvalidManifest)
}

fn eof<R: Read + ?Sized>(reader: &mut R) -> Result<(), FileTransferError> {
    let mut sentinel = [0];
    if read(reader, &mut sentinel)? != 0 {
        return Err(FileTransferError::LengthMismatch);
    }
    Ok(())
}

pub(super) fn inspect<R: Read + ?Sized>(
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: String,
    reader: &mut R,
) -> Result<FileTransferManifest, FileTransferError> {
    validate_file_display_name(&display_name).map_err(|_| FileTransferError::InvalidManifest)?;
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut whole = Sha256::new();
    let mut chunk = Sha256::new();
    let mut chunk_length = 0;
    let mut byte_length = 0;
    let mut chunks = Vec::new();
    loop {
        // Stop exactly at both fixed chunk boundaries and the total bound before the next read.
        let requested = (FILE_TRANSFER_CHUNK_BYTES - chunk_length)
            .min(MAX_FILE_TRANSFER_BYTES - byte_length)
            .min(FILE_TRANSFER_BUFFER_BYTES as u64) as usize;
        if requested == 0 {
            eof(reader)?;
            break;
        }
        let count = read(reader, &mut buffer[..requested])?;
        if count == 0 {
            if chunk_length != 0 {
                chunks.push(digest(chunk)?);
            }
            break;
        }
        whole.update(&buffer[..count]);
        chunk.update(&buffer[..count]);
        byte_length += count as u64;
        chunk_length += count as u64;
        if chunk_length == FILE_TRANSFER_CHUNK_BYTES {
            chunks.push(digest(chunk)?);
            chunk = Sha256::new();
            chunk_length = 0;
        }
    }
    FileTransferManifest::new(
        workspace,
        file_id,
        version_id,
        display_name,
        byte_length,
        chunks,
        digest(whole)?,
    )
    .map_err(|_| FileTransferError::InvalidManifest)
}

fn part<R: Read + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    mut byte_length: u64,
    buffer: &mut [u8; FILE_TRANSFER_BUFFER_BYTES],
    mut whole: Option<&mut Sha256>,
    mut writer: Option<&mut W>,
) -> Result<Sha256Digest, FileTransferError> {
    let mut chunk = Sha256::new();
    while byte_length != 0 {
        let requested = byte_length.min(FILE_TRANSFER_BUFFER_BYTES as u64) as usize;
        let count = read(reader, &mut buffer[..requested])?;
        if count == 0 {
            return Err(FileTransferError::LengthMismatch);
        }
        let bytes = &buffer[..count];
        chunk.update(bytes);
        if let Some(whole) = whole.as_deref_mut() {
            whole.update(bytes);
        }
        if let Some(writer) = writer.as_deref_mut() {
            write(writer, bytes)?;
        }
        byte_length -= count as u64;
    }
    digest(chunk)
}

fn check_chunk<R: Read + ?Sized, W: Write + ?Sized>(
    manifest: &FileTransferManifest,
    index: u32,
    reader: &mut R,
    writer: Option<&mut W>,
) -> Result<(), FileTransferError> {
    let spec = manifest
        .chunk(index)
        .map_err(|_| FileTransferError::InvalidChunk)?;
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let found = part(reader, spec.byte_length, &mut buffer, None, writer)?;
    eof(reader)?;
    if found != spec.sha256 {
        return Err(FileTransferError::DigestMismatch);
    }
    Ok(())
}

pub(super) fn verify_chunk<R: Read + ?Sized>(
    manifest: &FileTransferManifest,
    index: u32,
    reader: &mut R,
) -> Result<(), FileTransferError> {
    check_chunk(manifest, index, reader, None::<&mut std::io::Sink>)
}

pub(super) fn copy_chunk<R: Read + ?Sized, W: Write + ?Sized>(
    manifest: &FileTransferManifest,
    index: u32,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), FileTransferError> {
    check_chunk(manifest, index, reader, Some(writer))
}

pub(super) fn verify<R: Read + ?Sized>(
    manifest: &FileTransferManifest,
    reader: &mut R,
) -> Result<(), FileTransferError> {
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut whole = Sha256::new();
    for index in 0..manifest.chunk_sha256().len() {
        let spec = manifest
            .chunk(index as u32)
            .map_err(|_| FileTransferError::InvalidManifest)?;
        let found = part(
            reader,
            spec.byte_length,
            &mut buffer,
            Some(&mut whole),
            None::<&mut std::io::Sink>,
        )?;
        if found != spec.sha256 {
            return Err(FileTransferError::DigestMismatch);
        }
    }
    eof(reader)?;
    if digest(whole)? != *manifest.sha256() {
        return Err(FileTransferError::DigestMismatch);
    }
    Ok(())
}
