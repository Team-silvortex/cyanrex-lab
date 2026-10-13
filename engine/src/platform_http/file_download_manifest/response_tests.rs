use super::*;
use crate::models::collaboration::{
    FileId, FileVersionId, Sha256Digest, WorkspaceRef, FILE_TRANSFER_CHUNK_BYTES,
    MAX_FILE_TRANSFER_BYTES, MAX_FILE_TRANSFER_CHUNKS,
};
use axum::{body::to_bytes, response::IntoResponse};

fn selected(length: u64, chunks: usize, name: &str) -> FileTransferManifest {
    let workspace = WorkspaceRef {
        authority_id: "00000000-0000-4000-8000-0000000000b1".parse().unwrap(),
        workspace_id: "00000000-0000-4000-8000-0000000000b2".parse().unwrap(),
    };
    let file_id: FileId = "00000000-0000-4000-8000-0000000000a3".parse().unwrap();
    let version_id: FileVersionId = "00000000-0000-4000-8000-0000000000a4".parse().unwrap();
    let digest: Sha256Digest = "a".repeat(64).parse().unwrap();
    FileTransferManifest::new(
        workspace,
        file_id,
        version_id,
        name,
        length,
        vec![digest; chunks],
        "a".repeat(64).parse().unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn direct_manifest_projection_has_exact_byte_length_and_no_binary_or_authority_extras() {
    let selected = selected(17, 1, "🧪验收.bin");
    let response =
        manifest(&selected).unwrap_or_else(|_| panic!("valid typed manifest serializes"));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    let length = response.headers()[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    for header in [
        "content-disposition",
        "set-cookie",
        "etag",
        "last-modified",
        "content-range",
        "content-encoding",
        "access-control-allow-origin",
        "x-cyanrex-file-transfer-version",
        "x-cyanrex-file-id",
        "x-cyanrex-chunk-index",
    ] {
        assert!(!response.headers().contains_key(header));
    }
    let bytes = to_bytes(response.into_body(), MAX_FILE_TRANSFER_MANIFEST_BYTES)
        .await
        .unwrap();
    assert_eq!(length, bytes.len());
    assert_eq!(bytes.as_ref(), serde_json::to_vec(&selected).unwrap());
    assert_eq!(FileTransferManifest::parse_json(&bytes).unwrap(), selected);
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let actual = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual,
        [
            "schema_version",
            "workspace",
            "file_id",
            "version_id",
            "display_name",
            "byte_length",
            "chunk_size",
            "chunk_sha256",
            "sha256"
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
    );
    assert_eq!(json["schema_version"], 1);
}

#[tokio::test]
async fn maximum_typed_manifest_retains_all_ordered_digests_within_native_wire_budget() {
    let selected = selected(
        MAX_FILE_TRANSFER_BYTES,
        MAX_FILE_TRANSFER_CHUNKS,
        &"验".repeat(128),
    );
    assert_eq!(selected.chunk_size(), FILE_TRANSFER_CHUNK_BYTES);
    let response =
        manifest(&selected).unwrap_or_else(|_| panic!("maximum typed metadata must fit"));
    let length = response.headers()[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let bytes = to_bytes(response.into_body(), MAX_FILE_TRANSFER_MANIFEST_BYTES)
        .await
        .unwrap();
    assert_eq!(length, bytes.len());
    assert!(length <= MAX_FILE_TRANSFER_MANIFEST_BYTES);
    assert_eq!(FileTransferManifest::parse_json(&bytes).unwrap(), selected);
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json["chunk_sha256"].as_array().unwrap().len(),
        MAX_FILE_TRANSFER_CHUNKS
    );
}

#[tokio::test]
async fn serialized_wire_guard_accepts_exact_limit_and_rejects_one_extra_byte_privately() {
    assert_eq!(
        checked_bytes(vec![0; MAX_FILE_TRANSFER_MANIFEST_BYTES])
            .unwrap_or_else(|_| panic!("exact bound is permitted"))
            .len(),
        MAX_FILE_TRANSFER_MANIFEST_BYTES
    );
    let error = match checked_bytes(vec![0; MAX_FILE_TRANSFER_MANIFEST_BYTES + 1]) {
        Err(error) => error,
        Ok(_) => panic!("extra wire byte must fail closed"),
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}})
    );
}
