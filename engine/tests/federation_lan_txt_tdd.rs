//! One raw DNS-SD TXT RDATA, not a DNS packet parser or network discovery acceptance.
use cyanrex_engine::services::federation_discovery::{
    lan::{
        decode_txt, encode_txt, CacheTime, LanCandidateCache, LanDiscoveryError, LanSource,
        MAX_TXT_CHUNKS, MAX_TXT_CHUNK_BYTES, MAX_TXT_RECORD_BYTES,
    },
    DiscoveryPolicy, PeerAdvertisement, ProtocolRange,
};
use serde_json::{json, Value};

fn advertisement() -> Value {
    json!({
        "schema_version":1, "node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3}, "offered_capabilities":["peer.identity"],
        "required_capabilities":[], "issued_at":1000, "expires_at":1100,
        "hints":[{"kind":"direct","address":"192.168.1.7:4433"}]
    })
}
fn peer(value: &Value) -> PeerAdvertisement {
    PeerAdvertisement::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}
fn record(parts: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for part in parts {
        assert!(part.len() <= 255);
        bytes.push(part.len() as u8);
        bytes.extend(part);
    }
    bytes
}
fn frame_data(bytes: &[u8]) -> Vec<u8> {
    let count = bytes.len().div_ceil(MAX_TXT_CHUNK_BYTES);
    let mut fields = vec![b"v=1".to_vec(), format!("c={count}").into_bytes()];
    for (index, part) in bytes.chunks(MAX_TXT_CHUNK_BYTES).enumerate() {
        let mut field = format!("d{index}=").into_bytes();
        field.extend(part);
        fields.push(field);
    }
    record(&fields)
}
fn parts(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let length = bytes[offset] as usize;
        result.push(bytes[offset + 1..offset + 1 + length].to_vec());
        offset += length + 1;
    }
    result
}

#[test]
fn txt_limits_are_smaller_than_f0_and_do_not_imply_a_multicast_listener() {
    assert_eq!(MAX_TXT_RECORD_BYTES, 1200);
    assert_eq!(MAX_TXT_CHUNK_BYTES, 200);
    assert_eq!(MAX_TXT_CHUNKS, 6);
}

#[test]
fn canonical_advertisement_roundtrips_through_one_bounded_txt_record() {
    let original = peer(&advertisement());
    let bytes = encode_txt(&original).unwrap();
    assert!(bytes.len() <= MAX_TXT_RECORD_BYTES);
    let fields = parts(&bytes);
    assert_eq!(fields[0], b"v=1");
    assert!(fields.len() <= 2 + MAX_TXT_CHUNKS);
    for field in &fields {
        assert!(field.len() <= 255);
    }
    assert_eq!(
        decode_txt(&bytes).unwrap().encode().unwrap(),
        original.encode().unwrap()
    );
}

#[test]
fn field_order_and_ascii_key_case_do_not_change_dns_sd_meaning() {
    let original = peer(&advertisement());
    let mut fields = parts(&encode_txt(&original).unwrap());
    for field in &mut fields {
        let key_end = field.iter().position(|b| *b == b'=').unwrap();
        field[..key_end].make_ascii_uppercase();
    }
    fields.reverse();
    assert_eq!(
        decode_txt(&record(&fields)).unwrap().encode().unwrap(),
        original.encode().unwrap()
    );
}

#[test]
fn empty_truncated_zero_length_or_oversized_records_fail_without_partial_output() {
    for bytes in [vec![], vec![0], vec![5, b'v', b'=', b'1'], vec![255; 1201]] {
        assert!(decode_txt(&bytes).is_err());
    }
    let bytes = encode_txt(&peer(&advertisement())).unwrap();
    for length in 1..bytes.len() {
        assert!(decode_txt(&bytes[..length]).is_err(), "prefix {length}");
    }
}

#[test]
fn duplicate_unknown_binary_and_missing_txt_keys_are_closed() {
    let original = parts(&encode_txt(&peer(&advertisement())).unwrap());
    for extra in [
        b"v=1".to_vec(),
        b"V=1".to_vec(),
        b"c=2".to_vec(),
        b"D0=private".to_vec(),
        b"cookie=private-secret".to_vec(),
        vec![0xff, b'=', 1],
        b"flag".to_vec(),
        b"=empty".to_vec(),
    ] {
        let mut fields = original.clone();
        fields.push(extra);
        assert!(decode_txt(&record(&fields)).is_err());
    }
    for missing in 0..original.len() {
        let mut fields = original.clone();
        fields.remove(missing);
        assert!(decode_txt(&record(&fields)).is_err());
    }
}

#[test]
fn version_chunk_count_and_chunk_size_are_exact_not_coerced() {
    let original = parts(&encode_txt(&peer(&advertisement())).unwrap());
    for version in ["v=0", "v=2", "v=01", "v=1 "] {
        let mut fields = original.clone();
        fields[0] = version.as_bytes().to_vec();
        assert!(decode_txt(&record(&fields)).is_err());
    }
    for count in ["c=0", "c=7", "c=01", "c=+2", "c=2 "] {
        let mut fields = original.clone();
        fields[1] = count.as_bytes().to_vec();
        assert!(decode_txt(&record(&fields)).is_err());
    }
    for field in [
        b"d0=".to_vec(),
        [b"d0=".as_slice(), &[b' '; 201]].concat(),
        b"d00=x".to_vec(),
        b"d6=x".to_vec(),
    ] {
        let mut fields = original.clone();
        fields[2] = field;
        assert!(decode_txt(&record(&fields)).is_err());
    }
    let mut fields = original;
    fields[2].pop();
    assert!(
        decode_txt(&record(&fields)).is_err(),
        "non-final chunks must have exact width"
    );
}

#[test]
fn oversized_valid_f0_is_rejected_not_truncated_or_split_across_records() {
    let mut value = advertisement();
    value["offered_capabilities"] = json!((0..16)
        .map(|i| format!("peer.c{i}{}", "a".repeat(95)))
        .collect::<Vec<_>>());
    let original = peer(&value);
    assert!(original.encode().unwrap().len() > MAX_TXT_RECORD_BYTES);
    assert!(matches!(
        encode_txt(&original),
        Err(LanDiscoveryError::TooLarge)
    ));
}

#[test]
fn raw_txt_budget_accepts_exact_limit_and_rejects_one_more_byte() {
    let original = peer(&advertisement());
    let mut bytes = original.encode().unwrap();
    // v/c framing: 8 bytes. Six dN length/key prefixes: 24 bytes.
    bytes.resize(MAX_TXT_RECORD_BYTES - 32, b' ');
    let exact = frame_data(&bytes);
    assert_eq!(exact.len(), MAX_TXT_RECORD_BYTES);
    assert_eq!(
        decode_txt(&exact).unwrap().encode().unwrap(),
        original.encode().unwrap()
    );
    bytes.push(b' ');
    assert!(matches!(
        decode_txt(&frame_data(&bytes)),
        Err(LanDiscoveryError::TooLarge)
    ));
}

#[test]
fn invalid_utf8_and_semantic_json_duplicates_do_not_bypass_f0_through_txt() {
    assert!(matches!(
        decode_txt(&frame_data(&[0xff])),
        Err(LanDiscoveryError::RejectedAdvertisement(_))
    ));
    let original = peer(&advertisement()).encode().unwrap();
    let mut duplicate = b"{\"schema_version\":1,".to_vec();
    duplicate.extend_from_slice(&original[1..]);
    assert!(matches!(
        decode_txt(&frame_data(&duplicate)),
        Err(LanDiscoveryError::RejectedAdvertisement(_))
    ));
}

#[test]
fn embedded_json_stays_f0_strict_and_errors_never_echo_txt_values() {
    let bytes = b"{\"private-cookie\":\"secret-value\"}";
    let fields = vec![
        b"v=1".to_vec(),
        b"c=1".to_vec(),
        [b"d0=".as_slice(), bytes].concat(),
    ];
    let error = decode_txt(&record(&fields)).unwrap_err();
    let output = format!("{error:?} {error}");
    assert!(!output.contains("private-cookie"));
    assert!(!output.contains("secret-value"));
    let valid = encode_txt(&peer(&advertisement())).unwrap();
    let mut two_records = valid.clone();
    two_records.extend(valid);
    assert!(
        decode_txt(&two_records).is_err(),
        "no cross-record assembly"
    );
}

#[test]
fn txt_to_cache_preserves_unverified_claims_and_original_expiry() {
    let policy = DiscoveryPolicy::new(
        "a".repeat(64).parse().unwrap(),
        ProtocolRange::new(1, 3).unwrap(),
        vec!["peer.identity".parse().unwrap()],
        vec![],
    )
    .unwrap();
    let mut cache = LanCandidateCache::new(policy, vec![17], 2).unwrap();
    let source = LanSource::new(17, "peer-local-1").unwrap();
    let decoded = decode_txt(&encode_txt(&peer(&advertisement())).unwrap()).unwrap();
    cache
        .observe(
            &source,
            &decoded.encode().unwrap(),
            60,
            CacheTime::new(1010, 10).unwrap(),
        )
        .unwrap();
    let candidates = cache.snapshot(CacheTime::new(1010, 10).unwrap()).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source(), &source);
    assert_eq!(
        candidates[0].candidate().claimed_node().as_str(),
        "b".repeat(64)
    );
    assert_eq!(candidates[0].deadline_monotonic(), 70);
    assert!(cache
        .snapshot(CacheTime::new(1070, 70).unwrap())
        .unwrap()
        .is_empty());
}
