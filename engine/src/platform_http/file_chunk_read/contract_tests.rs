use super::*;

const QUERY: &str = concat!(
    "grant_id=00000000-0000-4000-8000-0000000000a1&",
    "upload_id=00000000-0000-4000-8000-0000000000a2&",
    "file_id=00000000-0000-4000-8000-0000000000a3&",
    "version_id=00000000-0000-4000-8000-0000000000a4&",
    "byte_length=17&sha256=",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&chunk_index=0"
);
const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "00000000-0000-4000-8000-0000000000b1".parse().unwrap(),
        workspace_id: "00000000-0000-4000-8000-0000000000b2".parse().unwrap(),
    }
}

fn parse(query: &str) -> SessionFileChunkReadRequest {
    chunk_request(Some(query), scope()).unwrap_or_else(|_| panic!("canonical query must parse"))
}

#[test]
fn canonical_query_is_order_independent_and_uses_only_trusted_scope() {
    let selected = parse(QUERY);
    assert_eq!(selected.download.expected_file.workspace(), scope());
    assert_eq!(selected.download.expected_file.byte_length(), 17);
    assert_eq!(selected.chunk_index, 0);
    let reversed = QUERY.split('&').rev().collect::<Vec<_>>().join("&");
    let other = parse(&reversed);
    assert_eq!(other.download.grant_id, selected.download.grant_id);
    assert_eq!(other.download.upload_id, selected.download.upload_id);
    assert_eq!(
        other.download.expected_file,
        selected.download.expected_file
    );
    assert_eq!(other.chunk_index, selected.chunk_index);
}

#[test]
fn every_field_is_required_and_duplicates_unknowns_or_empty_pairs_fail_closed() {
    assert!(chunk_request(None, scope()).is_err());
    assert!(chunk_request(Some(""), scope()).is_err());
    let parts = QUERY.split('&').collect::<Vec<_>>();
    for omitted in 0..parts.len() {
        let missing = parts
            .iter()
            .enumerate()
            .filter_map(|(index, value)| (index != omitted).then_some(*value))
            .collect::<Vec<_>>()
            .join("&");
        assert!(chunk_request(Some(&missing), scope()).is_err(), "{missing}");
        let duplicate = format!("{QUERY}&{}", parts[omitted]);
        assert!(chunk_request(Some(&duplicate), scope()).is_err());
    }
    for bad in [
        format!("{QUERY}&owner=someone"),
        format!("{QUERY}&workspace=somewhere"),
        format!("{QUERY}&"),
        format!("&{QUERY}"),
        QUERY.replace("&upload_id=", "&&upload_id="),
        QUERY.replace("byte_length=17", "byte_length="),
        QUERY.replace("byte_length=17", "byte_length"),
        QUERY.replace("byte_length=17", "byte_length=17=17"),
    ] {
        assert!(chunk_request(Some(&bad), scope()).is_err(), "{bad}");
    }
}

#[test]
fn raw_query_rejects_encoded_plus_noncanonical_uuid_and_digest_aliases() {
    for key in ["grant_id", "upload_id", "file_id", "version_id"] {
        let value = QUERY
            .split('&')
            .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
            .unwrap();
        for changed in [
            value.to_ascii_uppercase(),
            value.replace('-', ""),
            "00000000-0000-0000-0000-000000000000".to_owned(),
            format!("{{{value}}}"),
            format!("urn:uuid:{value}"),
            value.replacen('0', "%30", 1),
            format!("+{value}"),
        ] {
            let bad = QUERY.replace(&format!("{key}={value}"), &format!("{key}={changed}"));
            assert!(chunk_request(Some(&bad), scope()).is_err(), "{bad}");
        }
    }
    for bad in [
        QUERY.replace("grant_id=", "%67rant_id="),
        QUERY.replace("chunk_index=0", "chunk_index=%30"),
        QUERY.replace("chunk_index=0", "chunk_index=+0"),
        QUERY.replace(&"a".repeat(64), &"A".repeat(64)),
        QUERY.replace(&"a".repeat(64), &"a".repeat(63)),
        QUERY.replace(&"a".repeat(64), &"g".repeat(64)),
    ] {
        assert!(chunk_request(Some(&bad), scope()).is_err(), "{bad}");
    }
}

#[test]
fn canonical_decimal_geometry_limits_and_empty_digest_are_checked_before_dispatch() {
    let maximum = QUERY
        .replace(
            "byte_length=17",
            &format!("byte_length={MAX_FILE_TRANSFER_BYTES}"),
        )
        .replace("chunk_index=0", "chunk_index=16383");
    assert_eq!(
        parse(&maximum).download.expected_file.byte_length(),
        MAX_FILE_TRANSFER_BYTES
    );
    assert_eq!(parse(&maximum).chunk_index, 16383);
    for field in ["byte_length", "chunk_index"] {
        let original = if field == "byte_length" { "17" } else { "0" };
        for value in [
            "00",
            "01",
            "017",
            "-1",
            "1.0",
            "1e1",
            " 1",
            "1 ",
            "18446744073709551616",
        ] {
            let bad = QUERY.replace(&format!("{field}={original}"), &format!("{field}={value}"));
            assert!(chunk_request(Some(&bad), scope()).is_err(), "{bad}");
        }
    }
    assert!(chunk_request(
        Some(&QUERY.replace("chunk_index=0", "chunk_index=16384")),
        scope()
    )
    .is_err());
    assert!(chunk_request(
        Some(&QUERY.replace(
            "byte_length=17",
            &format!("byte_length={}", MAX_FILE_TRANSFER_BYTES + 1)
        )),
        scope()
    )
    .is_err());
    let empty = QUERY.replace("byte_length=17", "byte_length=0");
    assert!(chunk_request(Some(&empty), scope()).is_err());
    let empty = empty.replace(&"a".repeat(64), EMPTY_SHA);
    assert_eq!(parse(&empty).download.expected_file.byte_length(), 0);
    // Local manifest geometry still belongs to the Source; a globally bounded selector is inert.
    assert_eq!(
        parse(&empty.replace("chunk_index=0", "chunk_index=1")).chunk_index,
        1
    );
}

#[test]
fn raw_query_byte_budget_does_not_allocate_or_decode_unbounded_input() {
    let oversized = format!("{QUERY}&{}", "x".repeat(MAX_QUERY_BYTES + 1));
    assert!(chunk_request(Some(&oversized), scope()).is_err());
    let non_ascii = QUERY.replace("chunk_index=0", "chunk_index=０");
    assert!(chunk_request(Some(&non_ascii), scope()).is_err());
}
