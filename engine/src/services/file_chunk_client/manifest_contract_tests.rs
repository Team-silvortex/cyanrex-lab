use super::*;
use axum::http::HeaderValue;

fn response() -> Response<()> {
    Response::builder()
        .status(200)
        .header("content-length", "17")
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .header("pragma", "no-cache")
        .header("x-content-type-options", "nosniff")
        .body(())
        .unwrap()
}

#[test]
fn manifest_head_requires_exact_private_singletons_and_200() {
    assert_eq!(validate(&response()), Ok(17));
    for name in [
        "content-length",
        "content-type",
        "cache-control",
        "pragma",
        "x-content-type-options",
    ] {
        let mut missing = response();
        missing.headers_mut().remove(name);
        assert_eq!(
            validate(&missing),
            Err(FileManifestClientError::InvalidResponse)
        );
        let mut duplicate = response();
        duplicate
            .headers_mut()
            .append(name, response().headers()[name].clone());
        assert_eq!(
            validate(&duplicate),
            Err(FileManifestClientError::InvalidResponse)
        );
    }
    for status in [204, 206, 301, 304, 401, 403, 404, 500, 503] {
        let mut invalid = response();
        *invalid.status_mut() = StatusCode::from_u16(status).unwrap();
        assert_eq!(
            validate(&invalid),
            Err(FileManifestClientError::InvalidResponse)
        );
    }
    let mut mime = response();
    mime.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    assert_eq!(
        validate(&mime),
        Err(FileManifestClientError::InvalidResponse)
    );
}

#[test]
fn manifest_length_is_canonical_positive_and_independently_bounded() {
    for length in [
        "0",
        "00",
        "017",
        "+17",
        "-17",
        " 17",
        "17 ",
        "0x11",
        "1e1",
        "184467440737095516160",
    ] {
        let mut invalid = response();
        invalid
            .headers_mut()
            .insert("content-length", HeaderValue::from_str(length).unwrap());
        assert_eq!(
            validate(&invalid),
            Err(FileManifestClientError::InvalidResponse)
        );
    }
    for length in [1, MAX_FILE_TRANSFER_MANIFEST_BYTES] {
        let mut valid = response();
        valid.headers_mut().insert(
            "content-length",
            HeaderValue::from_str(&length.to_string()).unwrap(),
        );
        assert_eq!(validate(&valid), Ok(length));
    }
    let mut too_large = response();
    too_large.headers_mut().insert(
        "content-length",
        HeaderValue::from_str(&(MAX_FILE_TRANSFER_MANIFEST_BYTES + 1).to_string()).unwrap(),
    );
    assert_eq!(
        validate(&too_large),
        Err(FileManifestClientError::InvalidResponse)
    );
}

#[test]
fn manifest_head_rejects_transforms_credentials_cors_and_binary_metadata() {
    for name in [
        "content-encoding",
        "content-range",
        "transfer-encoding",
        "trailer",
        "set-cookie",
        "location",
        "etag",
        "last-modified",
        "retry-after",
        "content-disposition",
        "x-cyanrex-file-transfer-version",
        "x-cyanrex-file-id",
        "x-cyanrex-file-version-id",
        "x-cyanrex-chunk-index",
        "x-cyanrex-chunk-sha256",
        "x-cyanrex-file-sha256",
        "access-control-allow-origin",
        "access-control-expose-headers",
        "access-control-future-field",
    ] {
        let mut invalid = response();
        invalid
            .headers_mut()
            .insert(name, HeaderValue::from_static("ignored"));
        assert_eq!(
            validate(&invalid),
            Err(FileManifestClientError::InvalidResponse),
            "{name}"
        );
    }
    let mut bounded_other = response();
    bounded_other.headers_mut().insert(
        "x-untrusted-label",
        HeaderValue::from_static("../../not-a-path"),
    );
    assert_eq!(validate(&bounded_other), Ok(17));
}

#[test]
fn manifest_header_entry_count_and_byte_budget_count_repeated_values() {
    let mut entries = response();
    for _ in 0..59 {
        entries
            .headers_mut()
            .append("x-other", HeaderValue::from_static("v"));
    }
    assert_eq!(entries.headers().len(), 64);
    assert_eq!(validate(&entries), Ok(17));
    entries
        .headers_mut()
        .append("x-other", HeaderValue::from_static("v"));
    assert_eq!(
        validate(&entries),
        Err(FileManifestClientError::InvalidResponse)
    );

    let mut bytes = response();
    let used: usize = bytes
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.as_bytes().len())
        .sum();
    let padding = MAX_HEADER_BYTES - used - "x-padding".len();
    bytes.headers_mut().insert(
        "x-padding",
        HeaderValue::from_str(&"a".repeat(padding)).unwrap(),
    );
    assert_eq!(validate(&bytes), Ok(17));
    bytes.headers_mut().insert(
        "x-padding",
        HeaderValue::from_str(&"a".repeat(padding + 1)).unwrap(),
    );
    assert_eq!(
        validate(&bytes),
        Err(FileManifestClientError::InvalidResponse)
    );
}
