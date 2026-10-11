use super::*;

pub const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: Uuid::from_u128(0xa1).try_into().unwrap(),
        workspace_id: Uuid::from_u128(0xb1).try_into().unwrap(),
    }
}
pub fn account(seed: u128) -> FileAccountRef {
    FileAccountRef::new(
        PrincipalRef {
            authority_id: scope().authority_id,
            principal_id: Uuid::from_u128(0xc0 + seed).try_into().unwrap(),
        },
        Uuid::from_u128(0xd0 + seed).try_into().unwrap(),
    )
}
pub fn grant(seed: u128) -> FileGrantId {
    Uuid::from_u128(0xe0 + seed).to_string().parse().unwrap()
}
pub fn revision(value: u64) -> RevisionNumber {
    value.try_into().unwrap()
}
pub fn validity() -> FileGrantValidity {
    FileGrantValidity::new(100, 200).unwrap()
}
pub fn manifest(length: u64) -> FileTransferManifest {
    let sha: Sha256Digest = if length == 0 {
        EMPTY_SHA.to_owned()
    } else {
        "a".repeat(64)
    }
    .parse()
    .unwrap();
    let count = (length / FILE_TRANSFER_CHUNK_BYTES
        + u64::from(length % FILE_TRANSFER_CHUNK_BYTES != 0)) as usize;
    FileTransferManifest::new(
        scope(),
        Uuid::from_u128(0xf1).try_into().unwrap(),
        Uuid::from_u128(0xf2).try_into().unwrap(),
        "claimed.bin",
        length,
        vec![sha.clone(); count],
        sha,
    )
    .unwrap()
}
pub fn file() -> FileVersionRef {
    FileVersionRef::from_manifest(&manifest(11))
}
pub fn publisher(state: FileGrantState) -> FilePublisherGrant {
    FilePublisherGrant::new(
        grant(1),
        revision(3),
        scope(),
        account(1),
        validity(),
        state,
    )
    .unwrap()
}
pub fn recipient(state: FileGrantState) -> FileRecipientGrant {
    FileRecipientGrant::new(
        grant(2),
        revision(5),
        file(),
        account(1),
        account(2),
        validity(),
        state,
    )
    .unwrap()
}
pub fn raw_publisher() -> Value {
    json!({"schema_version":1,"id":grant(1).to_string(),"revision":3,"workspace":scope(),
        "subject":{"principal":account(1).principal(),"account_id":account(1).account_id()},
        "validity":{"not_before":100,"expires_at":200},"state":"active"})
}
pub fn raw_recipient() -> Value {
    json!({"schema_version":1,"id":grant(2).to_string(),"revision":5,
        "file":{"workspace":scope(),"file_id":manifest(11).file_id(),"version_id":manifest(11).version_id(),
            "byte_length":11,"sha256":"a".repeat(64)},
        "publisher":{"principal":account(1).principal(),"account_id":account(1).account_id()},
        "recipient":{"principal":account(2).principal(),"account_id":account(2).account_id()},
        "validity":{"not_before":100,"expires_at":200},"state":"active"})
}
pub fn reject_publisher(raw: &Value) {
    let bytes = serde_json::to_vec(raw).unwrap();
    assert!(
        FilePublisherGrant::parse_json(&bytes).is_err(),
        "parse accepted {raw}"
    );
    assert!(
        serde_json::from_slice::<FilePublisherGrant>(&bytes).is_err(),
        "serde accepted {raw}"
    );
}
pub fn reject_recipient(raw: &Value) {
    let bytes = serde_json::to_vec(raw).unwrap();
    assert!(
        FileRecipientGrant::parse_json(&bytes).is_err(),
        "parse accepted {raw}"
    );
    assert!(
        serde_json::from_slice::<FileRecipientGrant>(&bytes).is_err(),
        "serde accepted {raw}"
    );
}
