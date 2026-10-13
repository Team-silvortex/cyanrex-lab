use super::*;

const QUERY: &str = concat!(
    "grant_id=00000000-0000-4000-8000-0000000000a1&",
    "upload_id=00000000-0000-4000-8000-0000000000a2&",
    "file_id=00000000-0000-4000-8000-0000000000a3&",
    "version_id=00000000-0000-4000-8000-0000000000a4&",
    "byte_length=17&sha256=",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
);
const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub(super) fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "00000000-0000-4000-8000-0000000000b1".parse().unwrap(),
        workspace_id: "00000000-0000-4000-8000-0000000000b2".parse().unwrap(),
    }
}
fn parse(query: &str) -> SessionFileDownloadRequest {
    download_request(Some(query), scope()).unwrap_or_else(|_| panic!("canonical query must parse"))
}

#[test]
fn canonical_selectors_are_order_independent_and_use_only_trusted_scope() {
    let selected = parse(QUERY);
    assert_eq!(selected.expected_file.workspace(), scope());
    assert_eq!(selected.expected_file.byte_length(), 17);
    let reversed = QUERY.split('&').rev().collect::<Vec<_>>().join("&");
    let other = parse(&reversed);
    assert_eq!(other.grant_id, selected.grant_id);
    assert_eq!(other.upload_id, selected.upload_id);
    assert_eq!(other.expected_file, selected.expected_file);
}

#[test]
fn exact_six_fields_reject_missing_duplicates_unknowns_and_empty_pairs() {
    assert!(download_request(None, scope()).is_err());
    assert!(download_request(Some(""), scope()).is_err());
    let parts = QUERY.split('&').collect::<Vec<_>>();
    assert_eq!(parts.len(), 6);
    for omitted in 0..parts.len() {
        let missing = parts
            .iter()
            .enumerate()
            .filter_map(|(index, part)| (index != omitted).then_some(*part))
            .collect::<Vec<_>>()
            .join("&");
        assert!(download_request(Some(&missing), scope()).is_err());
        assert!(download_request(Some(&format!("{QUERY}&{}", parts[omitted])), scope()).is_err());
        let key = parts[omitted].split_once('=').unwrap().0;
        assert!(download_request(
            Some(&QUERY.replace(parts[omitted], &format!("{key}="))),
            scope()
        )
        .is_err());
    }
    for extra in [
        "chunk_index=0",
        "owner=someone",
        "scope=somewhere",
        "path=/tmp/file",
        "publisher=someone",
    ] {
        assert!(download_request(Some(&format!("{QUERY}&{extra}")), scope()).is_err());
    }
    for bad in [
        format!("{QUERY}&"),
        format!("&{QUERY}"),
        QUERY.replace('&', "&&"),
        QUERY.replace("byte_length=17", "byte_length"),
        QUERY.replace("byte_length=17", "byte_length=17=17"),
    ] {
        assert!(download_request(Some(&bad), scope()).is_err());
    }
}

#[test]
fn raw_selectors_reject_percent_plus_uuid_and_digest_aliases() {
    for key in ["grant_id", "upload_id", "file_id", "version_id"] {
        let value = QUERY
            .split('&')
            .find_map(|part| part.strip_prefix(&format!("{key}=")))
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
            assert!(download_request(Some(&bad), scope()).is_err());
        }
    }
    for bad in [
        QUERY.replace("grant_id=", "%67rant_id="),
        QUERY.replace("byte_length=17", "byte_length=%31%37"),
        QUERY.replace("byte_length=17", "byte_length=+17"),
        QUERY.replace(&"a".repeat(64), &"A".repeat(64)),
        QUERY.replace(&"a".repeat(64), &"a".repeat(63)),
        QUERY.replace(&"a".repeat(64), &"g".repeat(64)),
    ] {
        assert!(download_request(Some(&bad), scope()).is_err());
    }
}

#[test]
fn canonical_decimal_file_bound_and_empty_digest_are_enforced() {
    assert_eq!(
        parse(&QUERY.replace(
            "byte_length=17",
            &format!("byte_length={MAX_FILE_TRANSFER_BYTES}")
        ))
        .expected_file
        .byte_length(),
        MAX_FILE_TRANSFER_BYTES
    );
    for value in [
        "00",
        "01",
        "017",
        "-1",
        "1.0",
        "1e1",
        " 1",
        "1 ",
        "１７",
        "18446744073709551616",
    ] {
        assert!(download_request(
            Some(&QUERY.replace("byte_length=17", &format!("byte_length={value}"))),
            scope()
        )
        .is_err());
    }
    assert!(download_request(
        Some(&QUERY.replace(
            "byte_length=17",
            &format!("byte_length={}", MAX_FILE_TRANSFER_BYTES + 1)
        )),
        scope()
    )
    .is_err());
    let empty = QUERY.replace("byte_length=17", "byte_length=0");
    assert!(download_request(Some(&empty), scope()).is_err());
    assert_eq!(
        parse(&empty.replace(&"a".repeat(64), EMPTY_SHA))
            .expected_file
            .byte_length(),
        0
    );
}

#[test]
fn raw_query_limit_rejects_oversize_input_without_decode_or_projection() {
    let query = format!("{QUERY}&{}", "x".repeat(MAX_QUERY_BYTES + 1));
    assert!(query.len() > MAX_QUERY_BYTES);
    assert!(download_request(Some(&query), scope()).is_err());
}
