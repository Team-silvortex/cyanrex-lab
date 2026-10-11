use super::fixture::*;
use cyanrex_engine::models::collaboration::*;

fn reject(value: &serde_json::Value) {
    let bytes = serde_json::to_vec(value).unwrap();
    assert!(
        FileTransferManifest::parse_json(&bytes).is_err(),
        "accepted {value}"
    );
    assert!(
        serde_json::from_slice::<FileTransferManifest>(&bytes).is_err(),
        "serde accepted {value}"
    );
}

fn construct(
    length: u64,
    hashes: Vec<Sha256Digest>,
) -> Result<FileTransferManifest, ContractError> {
    let (file_id, version_id) = ids();
    let whole = if hashes.len() == 1 {
        hashes[0].clone()
    } else {
        digest(b"independent whole")
    };
    FileTransferManifest::new(
        super::scope(),
        file_id,
        version_id,
        "data.bin",
        length,
        hashes,
        whole,
    )
}

#[test]
fn exact_chunk_layout_covers_empty_partial_boundary_and_maximum() {
    assert_eq!(FILE_TRANSFER_CHUNK_BYTES, 4 * 1024 * 1024);
    assert_eq!(MAX_FILE_TRANSFER_BYTES, 64 * 1024 * 1024 * 1024);
    assert_eq!(MAX_FILE_TRANSFER_CHUNKS, 16_384);
    let empty = small_manifest(b"");
    assert_eq!(empty.byte_length(), 0);
    assert!(empty.chunk_sha256().is_empty());
    assert!(empty.chunk(0).is_err());
    for (length, count, last_length) in [
        (1, 1, 1),
        (FILE_TRANSFER_CHUNK_BYTES, 1, FILE_TRANSFER_CHUNK_BYTES),
        (FILE_TRANSFER_CHUNK_BYTES + 17, 2, 17),
        (
            MAX_FILE_TRANSFER_BYTES,
            MAX_FILE_TRANSFER_CHUNKS,
            FILE_TRANSFER_CHUNK_BYTES,
        ),
    ] {
        let hashes = vec![digest(b"repeated chunks are valid metadata"); count];
        let manifest = construct(length, hashes.clone()).unwrap();
        assert_eq!(manifest.byte_length(), length);
        assert_eq!(manifest.chunk_sha256(), hashes);
        let index = (count - 1) as u32;
        let chunk = manifest.chunk(index).unwrap();
        assert_eq!(chunk.index, index);
        assert_eq!(chunk.offset, u64::from(index) * FILE_TRANSFER_CHUNK_BYTES);
        assert_eq!(chunk.byte_length, last_length);
        assert_eq!(chunk.sha256, hashes[index as usize]);
        assert!(manifest.chunk(count as u32).is_err());
        assert!(manifest.chunk(u32::MAX).is_err());
    }
}

#[test]
fn constructor_rejects_invalid_counts_overflow_and_one_chunk_hash_disagreement() {
    let hash = digest(b"x");
    for (length, count) in [
        (0, 1),
        (1, 0),
        (1, 2),
        (FILE_TRANSFER_CHUNK_BYTES, 2),
        (FILE_TRANSFER_CHUNK_BYTES + 1, 1),
        (MAX_FILE_TRANSFER_BYTES + 1, 16_384),
        (u64::MAX, 16_384),
        (MAX_FILE_TRANSFER_BYTES, 16_385),
    ] {
        assert!(
            construct(length, vec![hash.clone(); count]).is_err(),
            "length {length}, chunks {count}"
        );
    }
    let (file_id, version_id) = ids();
    assert!(FileTransferManifest::new(
        super::scope(),
        file_id,
        version_id,
        "data.bin",
        1,
        vec![hash.clone()],
        digest(b"y")
    )
    .is_err());
    assert!(construct(1, vec![hash]).is_ok());
}

#[test]
fn raw_json_roundtrip_has_only_transport_neutral_metadata() {
    let raw = raw_manifest();
    let manifest = parse(&raw).unwrap();
    let (file_id, version_id) = ids();
    assert_eq!(manifest.workspace(), super::scope());
    assert_eq!(manifest.file_id(), file_id);
    assert_eq!(manifest.version_id(), version_id);
    assert_eq!(manifest.display_name(), "data.bin");
    assert_eq!(manifest.byte_length(), 11);
    assert_eq!(manifest.sha256(), &digest(b"inert bytes"));
    assert_eq!(serde_json::to_value(&manifest).unwrap(), raw);
    assert_eq!(
        serde_json::from_value::<FileTransferManifest>(raw).unwrap(),
        manifest
    );
}

#[test]
fn every_required_field_rejects_missing_null_and_unknown_security_inputs() {
    let raw = raw_manifest();
    for key in raw.as_object().unwrap().keys() {
        let mut missing = raw.clone();
        missing.as_object_mut().unwrap().remove(key);
        reject(&missing);
        let mut null = raw.clone();
        null[key] = serde_json::Value::Null;
        reject(&null);
    }
    for key in [
        "owner_id",
        "principal",
        "path",
        "storage_path",
        "download_url",
        "approved",
        "permissions",
    ] {
        let mut unknown = raw.clone();
        unknown[key] = serde_json::json!("not a capability");
        reject(&unknown);
    }
    let mut unknown = raw.clone();
    unknown["workspace"]["owner_id"] = serde_json::json!("foreign");
    reject(&unknown);
    for key in ["authority_id", "workspace_id"] {
        let mut missing = raw.clone();
        missing["workspace"].as_object_mut().unwrap().remove(key);
        reject(&missing);
        let mut null = raw.clone();
        null["workspace"][key] = serde_json::Value::Null;
        reject(&null);
    }
}

#[test]
fn positional_maps_duplicate_keys_and_trailing_documents_are_rejected() {
    let raw = raw_manifest();
    reject(&serde_json::Value::Array(
        raw.as_object().unwrap().values().cloned().collect(),
    ));
    let mut nested = raw.clone();
    nested["workspace"] =
        serde_json::json!([super::scope().authority_id, super::scope().workspace_id]);
    reject(&nested);
    let encoded = serde_json::to_string(&raw).unwrap();
    let duplicate = format!("{{\"schema_version\":1,{}", &encoded[1..]);
    assert!(FileTransferManifest::parse_json(duplicate.as_bytes()).is_err());
    let authority = super::scope().authority_id.to_string();
    let nested_duplicate = encoded.replace(
        &format!("\"authority_id\":\"{authority}\""),
        &format!("\"authority_id\":\"{authority}\",\"authority_id\":\"{authority}\""),
    );
    assert_ne!(nested_duplicate, encoded);
    assert!(FileTransferManifest::parse_json(nested_duplicate.as_bytes()).is_err());
    assert!(FileTransferManifest::parse_json(format!("{encoded} {{}}").as_bytes()).is_err());
    assert!(FileTransferManifest::parse_json(b"").is_err());
}

#[test]
fn schema_fixed_chunk_size_and_raw_integer_types_are_strict() {
    let raw = raw_manifest();
    for (key, invalid) in [
        ("schema_version", serde_json::json!(0)),
        ("schema_version", serde_json::json!(2)),
        (
            "chunk_size",
            serde_json::json!(FILE_TRANSFER_CHUNK_BYTES - 1),
        ),
        (
            "chunk_size",
            serde_json::json!(FILE_TRANSFER_CHUNK_BYTES + 1),
        ),
        (
            "byte_length",
            serde_json::json!(MAX_FILE_TRANSFER_BYTES + 1),
        ),
    ] {
        let mut changed = raw.clone();
        changed[key] = invalid;
        reject(&changed);
    }
    let encoded = serde_json::to_string(&raw).unwrap();
    for (key, original) in [
        ("schema_version", "1"),
        ("byte_length", "11"),
        ("chunk_size", "4194304"),
    ] {
        for token in ["-1", "1.0", "1e0", "\"1\"", "true", "18446744073709551616"] {
            let changed = encoded.replace(
                &format!("\"{key}\":{original}"),
                &format!("\"{key}\":{token}"),
            );
            assert_ne!(changed, encoded);
            assert!(
                FileTransferManifest::parse_json(changed.as_bytes()).is_err(),
                "accepted {key}={token}"
            );
        }
    }
}

#[test]
fn typed_identifiers_require_canonical_lowercase_non_nil_uuid_strings() {
    assert!(FileId::try_from(uuid::Uuid::nil()).is_err());
    assert!(FileVersionId::try_from(uuid::Uuid::nil()).is_err());
    let valid = "abcdef01-2345-4678-9abc-def012345678";
    assert!(valid.parse::<FileId>().is_ok());
    assert!(valid.parse::<FileVersionId>().is_ok());
    let raw = raw_manifest();
    for invalid in [
        "00000000-0000-0000-0000-000000000000",
        "ABCDEF01-2345-4678-9ABC-DEF012345678",
        "abcdef01234546789abcdef012345678",
        "{abcdef01-2345-4678-9abc-def012345678}",
        "bad",
    ] {
        assert!(invalid.parse::<FileId>().is_err());
        assert!(invalid.parse::<FileVersionId>().is_err());
        for key in ["file_id", "version_id"] {
            let mut changed = raw.clone();
            changed[key] = serde_json::json!(invalid);
            reject(&changed);
        }
        for key in ["authority_id", "workspace_id"] {
            let mut changed = raw.clone();
            changed["workspace"][key] = serde_json::json!(invalid);
            reject(&changed);
        }
    }
}

#[test]
fn all_digests_require_exact_lowercase_hex_and_exact_chunk_count() {
    let raw = raw_manifest();
    for invalid in [
        "",
        "f",
        &"f".repeat(63),
        &"f".repeat(65),
        &"A".repeat(64),
        &"g".repeat(64),
    ] {
        for key in ["sha256", "chunk_sha256"] {
            let mut changed = raw.clone();
            changed[key] = if key == "sha256" {
                serde_json::json!(invalid)
            } else {
                serde_json::json!([invalid])
            };
            reject(&changed);
        }
    }
    for chunks in [
        serde_json::json!([]),
        serde_json::json!([null]),
        serde_json::json!([12]),
        serde_json::json!([digest(b"inert bytes"), digest(b"inert bytes")]),
        serde_json::json!({}),
    ] {
        let mut changed = raw.clone();
        changed["chunk_sha256"] = chunks;
        reject(&changed);
    }
    let mut disagreement = raw.clone();
    disagreement["sha256"] = serde_json::json!(digest(b"different whole file"));
    reject(&disagreement);
    assert!(parse(&raw).is_ok());
}

#[test]
fn portable_display_names_are_inert_preserved_unicode_not_paths() {
    let (file_id, version_id) = ids();
    for name in [
        "作业 数据.bin".to_owned(),
        "e\u{301}.bin".to_owned(),
        "CON".to_owned(),
        "é".repeat(128),
        "😀".repeat(64),
    ] {
        let mut raw = raw_manifest();
        raw["display_name"] = serde_json::json!(name);
        let parsed = parse(&raw).unwrap();
        assert_eq!(parsed.display_name(), name);
        let constructed = FileTransferManifest::new(
            super::scope(),
            file_id,
            version_id,
            name.clone(),
            1,
            vec![digest(b"x")],
            digest(b"x"),
        )
        .unwrap();
        assert_eq!(constructed.display_name(), name);
    }
    for name in [
        "",
        " ",
        " leading",
        "trailing ",
        "trailing.",
        ".",
        "..",
        "a/b",
        "a\\b",
        "a:b",
        "a<b",
        "a>b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        "a\nb",
        "a\0b",
        "a\u{202a}b",
        "a\u{202e}b",
        "a\u{2066}b",
        "a\u{2069}b",
        &"x".repeat(129),
        &"😀".repeat(65),
    ] {
        let mut raw = raw_manifest();
        raw["display_name"] = serde_json::json!(name);
        reject(&raw);
        assert!(
            FileTransferManifest::new(
                super::scope(),
                file_id,
                version_id,
                name,
                1,
                vec![digest(b"x")],
                digest(b"x")
            )
            .is_err(),
            "accepted name {name:?}"
        );
    }
}

#[test]
fn json_size_cap_is_inclusive_and_maximum_digest_inventory_is_bounded() {
    assert_eq!(MAX_FILE_TRANSFER_MANIFEST_BYTES, 1152 * 1024);
    let mut padded = serde_json::to_vec(&raw_manifest()).unwrap();
    padded.resize(MAX_FILE_TRANSFER_MANIFEST_BYTES, b' ');
    assert!(FileTransferManifest::parse_json(&padded).is_ok());
    padded.push(b' ');
    assert!(FileTransferManifest::parse_json(&padded).is_err());
    let maximum = construct(
        MAX_FILE_TRANSFER_BYTES,
        vec![digest(b"repeated chunks"); MAX_FILE_TRANSFER_CHUNKS],
    )
    .unwrap();
    let bytes = serde_json::to_vec(&maximum).unwrap();
    assert!(bytes.len() < MAX_FILE_TRANSFER_MANIFEST_BYTES);
    assert_eq!(FileTransferManifest::parse_json(&bytes).unwrap(), maximum);
    let mut oversized = serde_json::to_value(maximum).unwrap();
    oversized["chunk_sha256"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(digest(b"extra")));
    assert!(serde_json::to_vec(&oversized).unwrap().len() < MAX_FILE_TRANSFER_MANIFEST_BYTES);
    reject(&oversized);
    oversized["chunk_sha256"][MAX_FILE_TRANSFER_CHUNKS] = serde_json::json!({"not": "a digest"});
    let bytes = serde_json::to_vec(&oversized).unwrap();
    assert!(bytes.len() < MAX_FILE_TRANSFER_MANIFEST_BYTES);
    let error = serde_json::from_slice::<FileTransferManifest>(&bytes)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("too many chunks"),
        "extra item must be refused before digest decoding: {error}"
    );
}
