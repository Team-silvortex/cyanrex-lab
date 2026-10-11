use super::fixture::*;
use cyanrex_engine::{models::collaboration::*, services::file_transfer::*};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions, Permissions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn snapshot(path: &std::path::Path) -> (u64, u64, u64, u64, u32, i64, i64, i64, i64, Sha256Digest) {
    let metadata = std::fs::metadata(path).unwrap();
    let mut file = File::open(path).unwrap();
    let mut whole = Sha256::new();
    let mut buffer = [0u8; FILE_TRANSFER_BUFFER_BYTES];
    loop {
        let amount = file.read(&mut buffer).unwrap();
        if amount == 0 {
            break;
        }
        whole.update(&buffer[..amount]);
    }
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.nlink(),
        metadata.mode(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        format!("{:x}", whole.finalize()).parse().unwrap(),
    )
}

fn inspect(file: &mut File) -> Result<FileTransferManifest, FileTransferError> {
    let (file_id, version_id) = ids();
    inspect_file(super::scope(), file_id, version_id, "data.bin", file)
}

#[test]
fn real_exclusively_created_large_file_inspects_and_verifies_without_writes() {
    let length = FILE_TRANSFER_CHUNK_BYTES + 17;
    assert!(length > 1024 * 1024);
    let mut fixture = FileFixture::new(length);
    let before = snapshot(&fixture.path);
    let oracle = pattern_manifest(length);
    assert_eq!(before.2, length);
    assert_eq!(before.3, 1);
    assert_eq!(before.9, *oracle.sha256());
    let inspected = inspect(&mut fixture.file).unwrap();
    assert_eq!(inspected, oracle);
    assert_eq!(fixture.file.stream_position().unwrap(), length);
    assert_eq!(snapshot(&fixture.path), before);
    fixture.file.rewind().unwrap();
    verify_file(&oracle, &mut fixture.file).unwrap();
    assert_eq!(fixture.file.stream_position().unwrap(), length);
    assert_eq!(snapshot(&fixture.path), before);
    fixture
        .file
        .seek(SeekFrom::Start(FILE_TRANSFER_CHUNK_BYTES))
        .unwrap();
    let mut sink = PatternSink::new(FILE_TRANSFER_CHUNK_BYTES, 3);
    copy_chunk_checked(&oracle, 1, &mut fixture.file, &mut sink).unwrap();
    assert_eq!(sink.written, 17);
    assert_eq!(snapshot(&fixture.path), before);
}

#[test]
fn empty_regular_descriptor_is_a_real_valid_file() {
    let mut fixture = FileFixture::new(0);
    let before = snapshot(&fixture.path);
    let manifest = inspect(&mut fixture.file).unwrap();
    assert_eq!(manifest.byte_length(), 0);
    assert_eq!(manifest.sha256(), &digest(b""));
    fixture.file.rewind().unwrap();
    verify_file(&manifest, &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), before);
}

#[test]
fn preopened_descriptor_must_start_at_zero_and_is_not_rewound_by_wrapper() {
    let mut fixture = FileFixture::new(17);
    let oracle = pattern_manifest(17);
    let before = snapshot(&fixture.path);
    fixture.file.seek(SeekFrom::Start(1)).unwrap();
    assert!(inspect(&mut fixture.file).is_err());
    assert_eq!(fixture.file.stream_position().unwrap(), 1);
    assert!(verify_file(&oracle, &mut fixture.file).is_err());
    assert_eq!(fixture.file.stream_position().unwrap(), 1);
    assert_eq!(snapshot(&fixture.path), before);
    fixture.file.rewind().unwrap();
    verify_file(&oracle, &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), before);
}

#[test]
fn hardlinked_descriptor_is_rejected_before_read_and_restores_healthy_after_unlink() {
    let mut fixture = FileFixture::new(17);
    let link = fixture.directory.join("second-link.bin");
    std::fs::hard_link(&fixture.path, &link).unwrap();
    let before = snapshot(&fixture.path);
    assert_eq!(before.3, 2);
    assert!(inspect(&mut fixture.file).is_err());
    assert_eq!(fixture.file.stream_position().unwrap(), 0);
    assert!(verify_file(&pattern_manifest(17), &mut fixture.file).is_err());
    assert_eq!(fixture.file.stream_position().unwrap(), 0);
    assert_eq!(snapshot(&fixture.path), before);
    assert_eq!(snapshot(&link), before);
    std::fs::remove_file(&link).unwrap();
    let restored = snapshot(&fixture.path);
    assert_eq!(restored.3, 1);
    verify_file(&pattern_manifest(17), &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), restored);
}

#[test]
fn nonregular_directory_descriptors_are_not_file_payloads() {
    let mut fixture = FileFixture::new(17);
    let before = snapshot(&fixture.path);
    let mut directory = File::open(&fixture.directory).unwrap();
    assert!(!directory.metadata().unwrap().is_file());
    assert!(inspect(&mut directory).is_err());
    assert!(verify_file(&pattern_manifest(17), &mut directory).is_err());
    assert_eq!(snapshot(&fixture.path), before);
    verify_file(&pattern_manifest(17), &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), before);
}

#[test]
fn wrapper_checks_bytes_against_manifest_and_does_not_repair_changed_file() {
    let mut fixture = FileFixture::new(17);
    let oracle = pattern_manifest(17);
    fixture.file.write_all(&[255]).unwrap();
    fixture.file.rewind().unwrap();
    let changed = snapshot(&fixture.path);
    assert_ne!(changed.9, *oracle.sha256());
    assert!(verify_file(&oracle, &mut fixture.file).is_err());
    assert_eq!(snapshot(&fixture.path), changed);
    fixture.file.rewind().unwrap();
    fixture.file.write_all(&[pattern_byte(0)]).unwrap();
    fixture.file.rewind().unwrap();
    let restored = snapshot(&fixture.path);
    verify_file(&oracle, &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), restored);
    fixture.file.set_len(16).unwrap();
    fixture.file.rewind().unwrap();
    let shortened = snapshot(&fixture.path);
    assert!(verify_file(&oracle, &mut fixture.file).is_err());
    assert_eq!(snapshot(&fixture.path), shortened);
    fixture.file.seek(SeekFrom::Start(16)).unwrap();
    fixture.file.write_all(&[pattern_byte(16)]).unwrap();
    fixture.file.rewind().unwrap();
    let restored = snapshot(&fixture.path);
    verify_file(&oracle, &mut fixture.file).unwrap();
    assert_eq!(snapshot(&fixture.path), restored);
}

#[test]
fn descriptor_core_does_not_require_private_permissions_or_claim_path_identity() {
    let mut fixture = FileFixture::new(17);
    std::fs::set_permissions(&fixture.path, Permissions::from_mode(0o666)).unwrap();
    let before = snapshot(&fixture.path);
    assert_eq!(before.4 & 0o777, 0o666);
    let renamed = fixture.directory.join("original-inode.bin");
    std::fs::rename(&fixture.path, &renamed).unwrap();
    let mut replacement = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&fixture.path)
        .unwrap();
    replacement.write_all(b"different inode").unwrap();
    let original = snapshot(&renamed);
    let replaced = snapshot(&fixture.path);
    assert_eq!(original.1, before.1);
    assert_ne!(replaced.1, original.1);
    let inspected = inspect(&mut fixture.file).unwrap();
    assert_eq!(inspected, pattern_manifest(17));
    fixture.file.rewind().unwrap();
    verify_file(&inspected, &mut fixture.file).unwrap();
    assert_eq!(snapshot(&renamed), original);
    assert_eq!(snapshot(&fixture.path), replaced);
}

#[test]
fn sparse_oversized_descriptor_is_rejected_without_streaming_sixty_four_gib() {
    let mut fixture = FileFixture::new(0);
    fixture.file.set_len(MAX_FILE_TRANSFER_BYTES + 1).unwrap();
    let fields = |metadata: std::fs::Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.nlink(),
            metadata.blocks(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    let before = fields(fixture.file.metadata().unwrap());
    assert_eq!(before.2, MAX_FILE_TRANSFER_BYTES + 1);
    assert!(inspect(&mut fixture.file).is_err());
    assert_eq!(
        fixture.file.stream_position().unwrap(),
        0,
        "oversized descriptor must fail before IO"
    );
    assert_eq!(fields(fixture.file.metadata().unwrap()), before);
    // The exclusive sparse fixture has no allocated payload; never hash/read all its logical bytes.
    fixture.file.set_len(0).unwrap();
    let restored = snapshot(&fixture.path);
    assert_eq!(inspect(&mut fixture.file).unwrap().byte_length(), 0);
    assert_eq!(snapshot(&fixture.path), restored);
}
