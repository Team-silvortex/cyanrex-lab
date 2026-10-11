use super::{fixture::*, *};
use serde::{Deserialize, Serialize};

// Ordinary Serde struct controls intentionally allow sequences. Their field order matches the
// frozen raw records, proving that the rejected arrays are otherwise well-typed, valid metadata.
#[derive(Serialize, Deserialize)]
struct PlainPublisher {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    workspace: WorkspaceRef,
    subject: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}
#[derive(Serialize, Deserialize)]
struct PlainRecipient {
    schema_version: CoreSchemaVersion,
    id: FileGrantId,
    revision: RevisionNumber,
    file: FileVersionRef,
    publisher: FileAccountRef,
    recipient: FileAccountRef,
    validity: FileGrantValidity,
    state: FileGrantState,
}
#[derive(Serialize, Deserialize)]
struct PlainAccount {
    principal: PrincipalRef,
    account_id: LegacyAccountId,
}
#[derive(Serialize, Deserialize)]
struct PlainVersion {
    workspace: WorkspaceRef,
    file_id: FileId,
    version_id: FileVersionId,
    byte_length: u64,
    sha256: Sha256Digest,
}
#[derive(Serialize, Deserialize)]
struct PlainValidity {
    not_before: u64,
    expires_at: u64,
}

pub fn positional(raw: &Value) -> Value {
    let object = raw.as_object().unwrap();
    let keys: &[&str] = if object.contains_key("schema_version") {
        if object.contains_key("file") {
            &[
                "schema_version",
                "id",
                "revision",
                "file",
                "publisher",
                "recipient",
                "validity",
                "state",
            ]
        } else {
            &[
                "schema_version",
                "id",
                "revision",
                "workspace",
                "subject",
                "validity",
                "state",
            ]
        }
    } else if object.contains_key("file_id") {
        &[
            "workspace",
            "file_id",
            "version_id",
            "byte_length",
            "sha256",
        ]
    } else if object.contains_key("principal") {
        &["principal", "account_id"]
    } else if object.contains_key("principal_id") {
        &["authority_id", "principal_id"]
    } else if object.contains_key("workspace_id") {
        &["authority_id", "workspace_id"]
    } else {
        assert!(object.contains_key("not_before"));
        &["not_before", "expires_at"]
    };
    let candidate = Value::Array(keys.iter().map(|key| object[*key].clone()).collect());
    let restored = if object.contains_key("schema_version") {
        if object.contains_key("file") {
            serde_json::to_value(
                serde_json::from_value::<PlainRecipient>(candidate.clone()).unwrap(),
            )
            .unwrap()
        } else {
            serde_json::to_value(
                serde_json::from_value::<PlainPublisher>(candidate.clone()).unwrap(),
            )
            .unwrap()
        }
    } else if object.contains_key("file_id") {
        serde_json::to_value(serde_json::from_value::<PlainVersion>(candidate.clone()).unwrap())
            .unwrap()
    } else if object.contains_key("principal") {
        serde_json::to_value(serde_json::from_value::<PlainAccount>(candidate.clone()).unwrap())
            .unwrap()
    } else if object.contains_key("principal_id") {
        serde_json::to_value(serde_json::from_value::<PrincipalRef>(candidate.clone()).unwrap())
            .unwrap()
    } else if object.contains_key("workspace_id") {
        serde_json::to_value(serde_json::from_value::<WorkspaceRef>(candidate.clone()).unwrap())
            .unwrap()
    } else {
        serde_json::to_value(serde_json::from_value::<PlainValidity>(candidate.clone()).unwrap())
            .unwrap()
    };
    assert_eq!(
        restored, *raw,
        "ordinary struct sequence control must roundtrip exact valid metadata"
    );
    candidate
}

#[derive(Clone, Copy)]
pub enum Kind {
    Publisher,
    Recipient,
}
impl Kind {
    pub fn raw(self) -> Value {
        match self {
            Self::Publisher => raw_publisher(),
            Self::Recipient => raw_recipient(),
        }
    }
    pub fn reject(self, raw: &Value) {
        match self {
            Self::Publisher => reject_publisher(raw),
            Self::Recipient => reject_recipient(raw),
        }
    }
    pub fn reject_bytes(self, bytes: &[u8]) {
        match self {
            Self::Publisher => {
                assert!(
                    FilePublisherGrant::parse_json(bytes).is_err(),
                    "publisher parse accepted {}",
                    String::from_utf8_lossy(bytes)
                );
                assert!(
                    serde_json::from_slice::<FilePublisherGrant>(bytes).is_err(),
                    "publisher serde accepted malformed raw JSON"
                );
            }
            Self::Recipient => {
                assert!(
                    FileRecipientGrant::parse_json(bytes).is_err(),
                    "recipient parse accepted {}",
                    String::from_utf8_lossy(bytes)
                );
                assert!(
                    serde_json::from_slice::<FileRecipientGrant>(bytes).is_err(),
                    "recipient serde accepted malformed raw JSON"
                );
            }
        }
    }
}
pub fn at<'a>(raw: &'a Value, path: &str) -> &'a Value {
    if path.is_empty() {
        raw
    } else {
        raw.pointer(path).unwrap()
    }
}
pub fn at_mut<'a>(raw: &'a mut Value, path: &str) -> &'a mut Value {
    if path.is_empty() {
        raw
    } else {
        raw.pointer_mut(path).unwrap()
    }
}
pub fn paths(raw: &Value) -> Vec<String> {
    fn walk(raw: &Value, path: String, all: &mut Vec<String>) {
        if let Some(object) = raw.as_object() {
            all.push(path.clone());
            for (key, value) in object {
                walk(value, format!("{path}/{key}"), all);
            }
        }
    }
    let mut all = Vec::new();
    walk(raw, String::new(), &mut all);
    all
}
/// Raw substitution keeps duplicates intact; building a Value would silently collapse them.
pub fn substitute(raw: &Value, path: &str, changed: &str) -> String {
    let encoded = serde_json::to_string(raw).unwrap();
    let result = if path.is_empty() {
        changed.to_owned()
    } else {
        let (parent, key) = path.rsplit_once('/').unwrap();
        let object = serde_json::to_string(at(raw, parent)).unwrap();
        let original = format!(
            "{}:{}",
            serde_json::to_string(key).unwrap(),
            serde_json::to_string(at(raw, path)).unwrap()
        );
        let replacement = format!("{}:{changed}", serde_json::to_string(key).unwrap());
        let updated = object.replacen(&original, &replacement, 1);
        assert_ne!(updated, object, "child not found at {path}");
        encoded.replacen(&object, &updated, 1)
    };
    assert_ne!(result, encoded, "no raw replacement at {path}");
    result
}
