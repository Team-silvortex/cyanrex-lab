use super::super::assembly_test_support::{runtime, staged, Root};
use std::io::{Read, Seek};

#[test]
fn original_assembly_pread_readers_interleave_without_mutating_the_retained_shared_offset() {
    let root = Root::new();
    let runtime = runtime();
    let bytes = b"seventeen bytes!!";
    let (staging, manifest) = staged(&runtime, &root, bytes);
    let assembled = runtime
        .block_on(staging.assemble_staged_file(&manifest))
        .unwrap();
    let proof = assembled.proof();
    let mut descriptor = &proof.file;
    let original_offset = descriptor.stream_position().unwrap();
    assert_eq!(original_offset, manifest.byte_length());
    let mut left = proof.reader();
    let mut right = proof.reader();
    let mut part = [0; 5];
    assert_eq!(left.read(&mut part).unwrap(), 5);
    assert_eq!(&part, &bytes[..5]);
    assert_eq!(right.read(&mut part).unwrap(), 5);
    assert_eq!(&part, &bytes[..5]);
    assert_eq!(left.read(&mut part).unwrap(), 5);
    assert_eq!(&part, &bytes[5..10]);
    let mut right_tail = Vec::new();
    right.read_to_end(&mut right_tail).unwrap();
    assert_eq!(right_tail, bytes[5..]);
    let mut left_tail = Vec::new();
    left.read_to_end(&mut left_tail).unwrap();
    assert_eq!(left_tail, bytes[10..]);
    assert_eq!(descriptor.stream_position().unwrap(), original_offset);
    proof.verify(&staging.inner.directory, &manifest).unwrap();
}
