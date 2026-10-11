use super::*;
use crate::models::collaboration::{validate_file_display_name, MAX_FILE_TRANSFER_BYTES};
use std::{
    fs::{File, Metadata},
    io::Seek,
    os::unix::fs::MetadataExt,
};

#[derive(PartialEq, Eq)]
struct Observation {
    device: u64,
    inode: u64,
    byte_length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl Observation {
    fn capture(metadata: &Metadata) -> Result<Self, FileTransferError> {
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(FileTransferError::SourceChanged);
        }
        if metadata.len() > MAX_FILE_TRANSFER_BYTES {
            return Err(FileTransferError::LengthMismatch);
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            byte_length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }

    fn begin(file: &mut File, expected: Option<u64>) -> Result<Self, FileTransferError> {
        let observed = Self::capture(&file.metadata()?)?;
        if expected.is_some_and(|length| length != observed.byte_length) {
            return Err(FileTransferError::LengthMismatch);
        }
        if file.stream_position()? != 0 {
            return Err(FileTransferError::SourceChanged);
        }
        Ok(observed)
    }

    fn finish(&self, file: &mut File, byte_length: u64) -> Result<(), FileTransferError> {
        if *self != Self::capture(&file.metadata()?)? {
            return Err(FileTransferError::SourceChanged);
        }
        if byte_length != self.byte_length || file.stream_position()? != self.byte_length {
            return Err(FileTransferError::LengthMismatch);
        }
        Ok(())
    }
}

pub(super) fn inspect(
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    display_name: String,
    file: &mut File,
) -> Result<FileTransferManifest, FileTransferError> {
    validate_file_display_name(&display_name).map_err(|_| FileTransferError::InvalidManifest)?;
    let before = Observation::begin(file, None)?;
    let manifest = inspect_stream(workspace, file_id, version_id, display_name, file)?;
    before.finish(file, manifest.byte_length())?;
    Ok(manifest)
}

pub(super) fn verify(
    manifest: &FileTransferManifest,
    file: &mut File,
) -> Result<(), FileTransferError> {
    let before = Observation::begin(file, Some(manifest.byte_length()))?;
    verify_stream(manifest, file)?;
    before.finish(file, manifest.byte_length())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::OpenOptions, io::Write, path::PathBuf};

    struct TemporaryFile(PathBuf);

    impl TemporaryFile {
        fn create() -> (Self, File) {
            let path = std::env::temp_dir().join(format!(
                "cyanrex-file-transfer-snapshot-{}",
                uuid::Uuid::new_v4()
            ));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            (Self(path), file)
        }
    }

    impl Drop for TemporaryFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn snapshot_boundary_rejects_real_same_length_write() {
        let (path, mut writer) = TemporaryFile::create();
        writer.write_all(b"original").unwrap();
        writer.sync_all().unwrap();
        let mut reader = File::open(&path.0).unwrap();
        let before = Observation::begin(&mut reader, Some(8)).unwrap();
        // Exercise the actual snapshot boundary, not a claimed concurrent in-read race. Repeated
        // same-length writes tolerate timestamp granularity without sleeping or changing clocks.
        let mut changed = false;
        for byte in 0..=255 {
            writer.rewind().unwrap();
            writer.write_all(&[byte; 8]).unwrap();
            writer.sync_all().unwrap();
            let after = Observation::capture(&reader.metadata().unwrap()).unwrap();
            assert_eq!(after.byte_length, before.byte_length);
            assert_eq!(after.device, before.device);
            assert_eq!(after.inode, before.inode);
            if after != before {
                changed = true;
                break;
            }
        }
        assert!(
            changed,
            "same-length writes must expose a changed metadata snapshot"
        );
        assert!(matches!(
            before.finish(&mut reader, 8),
            Err(FileTransferError::SourceChanged)
        ));
    }

    #[test]
    fn snapshot_boundary_rejects_different_descriptor_identity() {
        let (_first_path, mut first) = TemporaryFile::create();
        let (_second_path, mut second) = TemporaryFile::create();
        let before = Observation::begin(&mut first, Some(0)).unwrap();
        assert!(matches!(
            before.finish(&mut second, 0),
            Err(FileTransferError::SourceChanged)
        ));
    }
}
