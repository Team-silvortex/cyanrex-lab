use super::fixture::*;
use cyanrex_engine::{models::collaboration::*, services::file_transfer::*};
use std::io::{self, Read, Write};

#[test]
fn inspection_crosses_real_chunk_boundary_with_bounded_partial_reads() {
    let length = FILE_TRANSFER_CHUNK_BYTES + 17;
    let oracle = pattern_manifest(length);
    let (file_id, version_id) = ids();
    for max_returned in [FILE_TRANSFER_BUFFER_BYTES, 8191] {
        let mut reader = PatternReader::new(0, length);
        reader.max_returned = max_returned;
        let inspected =
            inspect_stream(super::scope(), file_id, version_id, "data.bin", &mut reader).unwrap();
        assert_eq!(inspected, oracle);
        assert_eq!(reader.consumed, length);
        assert!(reader.calls > 1);
        assert!(reader.max_requested <= FILE_TRANSFER_BUFFER_BYTES);
        let mut reader = PatternReader::new(0, length);
        reader.max_returned = max_returned;
        verify_stream(&oracle, &mut reader).unwrap();
        assert_eq!(reader.consumed, length);
    }
}

#[test]
fn empty_stream_inspection_and_verification_use_empty_sha_and_no_chunks() {
    let (file_id, version_id) = ids();
    let manifest = inspect_stream(
        super::scope(),
        file_id,
        version_id,
        "empty.bin",
        &mut &b""[..],
    )
    .unwrap();
    assert_eq!(manifest.byte_length(), 0);
    assert_eq!(manifest.sha256(), &digest(b""));
    assert!(manifest.chunk_sha256().is_empty());
    verify_stream(&manifest, &mut &b""[..]).unwrap();
    assert!(matches!(
        verify_stream(&manifest, &mut &b"x"[..]),
        Err(FileTransferError::LengthMismatch)
    ));
    assert!(matches!(
        verify_chunk(&manifest, 0, &mut &b""[..]),
        Err(FileTransferError::InvalidChunk)
    ));
}

#[test]
fn exact_chunk_and_last_partial_copy_support_short_reads_and_short_writes() {
    let manifest = pattern_manifest(FILE_TRANSFER_CHUNK_BYTES + 17);
    for index in [0, 1] {
        let chunk = manifest.chunk(index).unwrap();
        let mut reader = PatternReader::new(chunk.offset, chunk.byte_length);
        reader.max_returned = 4093;
        verify_chunk(&manifest, index, &mut reader).unwrap();
        assert_eq!(reader.consumed, chunk.byte_length);
        let mut reader = PatternReader::new(chunk.offset, chunk.byte_length);
        reader.max_returned = 4093;
        let mut sink = PatternSink::new(chunk.offset, 113);
        copy_chunk_checked(&manifest, index, &mut reader, &mut sink).unwrap();
        assert_eq!(reader.consumed, chunk.byte_length);
        assert_eq!(sink.written, chunk.byte_length);
        assert!(sink.calls > 0);
    }
}

#[test]
fn checked_copy_appends_without_owning_flush_seek_truncate_or_commit() {
    let manifest = small_manifest(b"right");
    let mut destination = b"existing prefix:".to_vec();
    copy_chunk_checked(&manifest, 0, &mut &b"right"[..], &mut destination).unwrap();
    assert_eq!(destination, b"existing prefix:right");
    // A Write-only sink has no Seek/truncate/sync API; PatternSink also panics on flush.
    let manifest = pattern_manifest(17);
    let mut sink = PatternSink::new(0, 3);
    copy_chunk_checked(&manifest, 0, &mut PatternReader::new(0, 17), &mut sink).unwrap();
    assert_eq!(sink.written, 17);
}

#[test]
fn short_and_oversized_chunk_reads_fail_length_and_only_probe_one_extra_byte() {
    let manifest = pattern_manifest(17);
    for length in [0, 16, 18, 1000] {
        let mut reader = PatternReader::new(0, length);
        assert!(matches!(
            verify_chunk(&manifest, 0, &mut reader),
            Err(FileTransferError::LengthMismatch)
        ));
        assert_eq!(reader.consumed, length.min(18));
        let mut reader = PatternReader::new(0, length);
        let mut sink = PatternSink::new(0, 7);
        assert!(matches!(
            copy_chunk_checked(&manifest, 0, &mut reader, &mut sink),
            Err(FileTransferError::LengthMismatch)
        ));
        assert_eq!(reader.consumed, length.min(18));
        assert!(sink.written <= 17, "overflow sentinel must not be copied");
    }
    verify_chunk(&manifest, 0, &mut PatternReader::new(0, 17)).unwrap();
}

#[test]
fn whole_file_length_mismatch_does_not_consume_unbounded_surplus() {
    let manifest = pattern_manifest(FILE_TRANSFER_CHUNK_BYTES + 17);
    for length in [
        FILE_TRANSFER_CHUNK_BYTES + 16,
        FILE_TRANSFER_CHUNK_BYTES + 1000,
    ] {
        let mut reader = PatternReader::new(0, length);
        assert!(matches!(
            verify_stream(&manifest, &mut reader),
            Err(FileTransferError::LengthMismatch)
        ));
        assert_eq!(reader.consumed, length.min(manifest.byte_length() + 1));
    }
    verify_stream(
        &manifest,
        &mut PatternReader::new(0, manifest.byte_length()),
    )
    .unwrap();
}

#[test]
fn wrong_chunk_digest_fails_and_checked_copy_leaves_uncommitted_bytes() {
    let manifest = small_manifest(b"right");
    assert!(matches!(
        verify_chunk(&manifest, 0, &mut &b"wrong"[..]),
        Err(FileTransferError::DigestMismatch)
    ));
    let mut sink = Vec::new();
    assert!(matches!(
        copy_chunk_checked(&manifest, 0, &mut &b"wrong"[..], &mut sink),
        Err(FileTransferError::DigestMismatch)
    ));
    assert_eq!(
        sink, b"wrong",
        "arbitrary caller-owned Write cannot roll back bytes"
    );
    let mut healthy = Vec::new();
    copy_chunk_checked(&manifest, 0, &mut &b"right"[..], &mut healthy).unwrap();
    assert_eq!(healthy, b"right");
    assert!(matches!(
        verify_stream(&manifest, &mut &b"wrong"[..]),
        Err(FileTransferError::DigestMismatch)
    ));
}

struct SwappedChunks {
    consumed: u64,
}

impl Read for SwappedChunks {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        assert!(buffer.len() <= FILE_TRANSFER_BUFFER_BYTES);
        let length = 2 * FILE_TRANSFER_CHUNK_BYTES;
        let amount = ((length - self.consumed) as usize).min(buffer.len());
        for (index, byte) in buffer[..amount].iter_mut().enumerate() {
            let offset = self.consumed + index as u64;
            let swapped = if offset < FILE_TRANSFER_CHUNK_BYTES {
                offset + FILE_TRANSFER_CHUNK_BYTES
            } else {
                offset - FILE_TRANSFER_CHUNK_BYTES
            };
            *byte = pattern_byte(swapped);
        }
        self.consumed += amount as u64;
        Ok(amount)
    }
}

#[test]
fn identical_length_changed_chunk_order_is_not_a_verified_file() {
    let manifest = pattern_manifest(2 * FILE_TRANSFER_CHUNK_BYTES);
    assert_ne!(manifest.chunk_sha256()[0], manifest.chunk_sha256()[1]);
    assert!(matches!(
        verify_stream(&manifest, &mut SwappedChunks { consumed: 0 }),
        Err(FileTransferError::DigestMismatch)
    ));
    verify_stream(
        &manifest,
        &mut PatternReader::new(0, manifest.byte_length()),
    )
    .unwrap();
}

#[test]
fn whole_digest_is_recomputed_from_bytes_not_trusted_chunk_digests() {
    let healthy = pattern_manifest(FILE_TRANSFER_CHUNK_BYTES + 17);
    let mut raw = serde_json::to_value(&healthy).unwrap();
    raw["sha256"] = serde_json::json!(digest(b"wrong whole digest"));
    let wrong_whole = parse(&raw).unwrap();
    for index in [0, 1] {
        let chunk = wrong_whole.chunk(index).unwrap();
        verify_chunk(
            &wrong_whole,
            index,
            &mut PatternReader::new(chunk.offset, chunk.byte_length),
        )
        .unwrap();
    }
    assert!(matches!(
        verify_stream(
            &wrong_whole,
            &mut PatternReader::new(0, healthy.byte_length())
        ),
        Err(FileTransferError::DigestMismatch)
    ));
    verify_stream(&healthy, &mut PatternReader::new(0, healthy.byte_length())).unwrap();
}

#[test]
fn malformed_chunk_hash_is_detected_before_accepting_correct_whole_hash() {
    let healthy = pattern_manifest(FILE_TRANSFER_CHUNK_BYTES + 17);
    let mut raw = serde_json::to_value(&healthy).unwrap();
    raw["chunk_sha256"][0] = serde_json::json!(digest(b"incorrect chunk"));
    let wrong_chunk = parse(&raw).unwrap();
    assert_eq!(wrong_chunk.sha256(), healthy.sha256());
    assert!(matches!(
        verify_stream(
            &wrong_chunk,
            &mut PatternReader::new(0, healthy.byte_length())
        ),
        Err(FileTransferError::DigestMismatch)
    ));
    verify_stream(&healthy, &mut PatternReader::new(0, healthy.byte_length())).unwrap();
}

#[test]
fn invalid_chunk_indices_do_not_read_or_write_any_bytes() {
    let manifest = small_manifest(b"right");
    for index in [1, u32::MAX] {
        let mut reader = FailReader {
            kind: io::ErrorKind::Other,
            calls: 0,
        };
        let mut sink = FailSink {
            kind: io::ErrorKind::Other,
            calls: 0,
        };
        assert!(matches!(
            verify_chunk(&manifest, index, &mut reader),
            Err(FileTransferError::InvalidChunk)
        ));
        assert!(matches!(
            copy_chunk_checked(&manifest, index, &mut reader, &mut sink),
            Err(FileTransferError::InvalidChunk)
        ));
        assert_eq!(reader.calls, 0);
        assert_eq!(sink.calls, 0);
    }
}

#[test]
fn interrupted_and_failed_reads_are_not_silently_retried_or_accepted() {
    let manifest = small_manifest(b"right");
    let (file_id, version_id) = ids();
    for kind in [io::ErrorKind::Interrupted, io::ErrorKind::Other] {
        let mut reader = FailReader { kind, calls: 0 };
        let result = inspect_stream(super::scope(), file_id, version_id, "data.bin", &mut reader);
        match result {
            Err(FileTransferError::Io(error)) => assert_eq!(error.kind(), kind),
            other => panic!("expected inspection IO error, got {other:?}"),
        }
        assert_eq!(reader.calls, 1);
        let mut reader = FailReader { kind, calls: 0 };
        assert_io(verify_chunk(&manifest, 0, &mut reader), kind);
        assert_eq!(reader.calls, 1);
        let mut reader = FailReader { kind, calls: 0 };
        assert_io(verify_stream(&manifest, &mut reader), kind);
        assert_eq!(reader.calls, 1);
        let mut reader = FailReader { kind, calls: 0 };
        let mut sink = Vec::new();
        assert_io(
            copy_chunk_checked(&manifest, 0, &mut reader, &mut sink),
            kind,
        );
        assert_eq!(reader.calls, 1);
        assert!(sink.is_empty());
    }
    verify_stream(&manifest, &mut &b"right"[..]).unwrap();
}

#[test]
fn interrupted_failed_and_zero_writes_fail_without_retry_or_flush() {
    let manifest = small_manifest(b"right");
    for kind in [io::ErrorKind::Interrupted, io::ErrorKind::Other] {
        let mut sink = FailSink { kind, calls: 0 };
        assert_io(
            copy_chunk_checked(&manifest, 0, &mut &b"right"[..], &mut sink),
            kind,
        );
        assert_eq!(sink.calls, 1);
    }
    let manifest = pattern_manifest(17);
    let mut sink = PatternSink::new(0, 0);
    assert_io(
        copy_chunk_checked(&manifest, 0, &mut PatternReader::new(0, 17), &mut sink),
        io::ErrorKind::WriteZero,
    );
    assert_eq!(sink.calls, 1);
    assert_eq!(sink.written, 0);
    let mut healthy = PatternSink::new(0, 1);
    copy_chunk_checked(&manifest, 0, &mut PatternReader::new(0, 17), &mut healthy).unwrap();
    assert_eq!(healthy.written, 17);
}

struct PrefixThenError {
    failed: bool,
}

impl Read for PrefixThenError {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        self.failed = true;
        let amount = buffer.len().min(3);
        buffer[..amount].copy_from_slice(&b"rig"[..amount]);
        Ok(amount)
    }
}

#[test]
fn mid_stream_failure_never_turns_partial_sink_into_verified_success() {
    let manifest = small_manifest(b"right");
    let mut reader = PrefixThenError { failed: false };
    let mut sink = Vec::new();
    assert_io(
        copy_chunk_checked(&manifest, 0, &mut reader, &mut sink),
        io::ErrorKind::UnexpectedEof,
    );
    assert_eq!(sink, b"rig");
    let mut healthy = Vec::new();
    copy_chunk_checked(&manifest, 0, &mut &b"right"[..], &mut healthy).unwrap();
    assert_eq!(healthy, b"right");
}

#[test]
fn invalid_display_name_is_rejected_before_consuming_the_reader() {
    let (file_id, version_id) = ids();
    for name in ["", " leading", "../data.bin", "a\u{202e}b"] {
        let mut reader = FailReader {
            kind: io::ErrorKind::Other,
            calls: 0,
        };
        assert!(matches!(
            inspect_stream(super::scope(), file_id, version_id, name, &mut reader),
            Err(FileTransferError::InvalidManifest)
        ));
        assert_eq!(reader.calls, 0);
    }
    let manifest = inspect_stream(
        super::scope(),
        file_id,
        version_id,
        "data.bin",
        &mut &b"right"[..],
    )
    .unwrap();
    assert_eq!(manifest, small_manifest(b"right"));
}

struct EofProbeFailure {
    bytes: &'static [u8],
    position: usize,
}

impl Read for EofProbeFailure {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.position == self.bytes.len() {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let amount = (self.bytes.len() - self.position).min(buffer.len());
        buffer[..amount].copy_from_slice(&self.bytes[self.position..self.position + amount]);
        self.position += amount;
        Ok(amount)
    }
}

#[test]
fn eof_probe_failure_is_not_treated_as_verified_eof_after_correct_bytes() {
    let manifest = small_manifest(b"right");
    let reader = || EofProbeFailure {
        bytes: b"right",
        position: 0,
    };
    assert_io(
        verify_chunk(&manifest, 0, &mut reader()),
        io::ErrorKind::BrokenPipe,
    );
    assert_io(
        verify_stream(&manifest, &mut reader()),
        io::ErrorKind::BrokenPipe,
    );
    let mut sink = Vec::new();
    assert_io(
        copy_chunk_checked(&manifest, 0, &mut reader(), &mut sink),
        io::ErrorKind::BrokenPipe,
    );
    assert_eq!(sink, b"right");
    let (file_id, version_id) = ids();
    match inspect_stream(
        super::scope(),
        file_id,
        version_id,
        "data.bin",
        &mut reader(),
    ) {
        Err(FileTransferError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::BrokenPipe),
        other => panic!("expected inspection EOF-probe IO error, got {other:?}"),
    }
    verify_stream(&manifest, &mut &b"right"[..]).unwrap();
}

struct OverreportReader;

impl Read for OverreportReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        Ok(buffer.len() + 1)
    }
}

struct OverreportSink;

impl Write for OverreportSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len() + 1)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("must not flush")
    }
}

#[test]
fn overreporting_io_implementations_return_invalid_data_without_panicking() {
    let manifest = small_manifest(b"right");
    assert_io(
        verify_chunk(&manifest, 0, &mut OverreportReader),
        io::ErrorKind::InvalidData,
    );
    assert_io(
        verify_stream(&manifest, &mut OverreportReader),
        io::ErrorKind::InvalidData,
    );
    assert_io(
        copy_chunk_checked(&manifest, 0, &mut OverreportReader, &mut Vec::new()),
        io::ErrorKind::InvalidData,
    );
    assert_io(
        copy_chunk_checked(&manifest, 0, &mut &b"right"[..], &mut OverreportSink),
        io::ErrorKind::InvalidData,
    );
    let (file_id, version_id) = ids();
    match inspect_stream(
        super::scope(),
        file_id,
        version_id,
        "data.bin",
        &mut OverreportReader,
    ) {
        Err(FileTransferError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::InvalidData),
        other => panic!("expected invalid-data inspection error, got {other:?}"),
    }
    let mut reader: &mut dyn Read = &mut &b"right"[..];
    let mut sink = Vec::new();
    let writer: &mut dyn Write = &mut sink;
    copy_chunk_checked(&manifest, 0, &mut reader, writer).unwrap();
    assert_eq!(sink, b"right");
}
