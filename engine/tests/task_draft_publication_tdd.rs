use chrono::{TimeZone, Utc};
use cyanrex_engine::{
    models::collaboration::*,
    services::task_draft_import::{TaskDraftPublicationPlan, MAX_TASK_DRAFT_BYTES},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[path = "task_draft_publication/binding.rs"]
mod binding;
#[path = "task_draft_publication/parsing.rs"]
mod parsing;

fn item(index: usize, text: &str) -> Value {
    json!({"id":format!("local_{index}"),"revision":1,"kind":"text",
        "filename":"note.txt","language":"plaintext","text":text})
}

fn draft(payload: Vec<Value>) -> Value {
    json!({"format":"cyanrex.task-draft","version":1,"title":"  Task title  ","payload":payload})
}

fn parse(value: &Value) -> Result<TaskDraftPublicationPlan, ContractError> {
    TaskDraftPublicationPlan::parse_json(&serde_json::to_vec(value).unwrap())
}

fn plan(texts: &[&str]) -> TaskDraftPublicationPlan {
    parse(&draft(
        texts
            .iter()
            .enumerate()
            .map(|(i, text)| item(i, text))
            .collect(),
    ))
    .ok()
    .expect("valid publication plan")
}

fn id<T: TryFrom<uuid::Uuid>>(value: u128) -> T
where
    T::Error: std::fmt::Debug,
{
    uuid::Uuid::from_u128(value).try_into().unwrap()
}

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: id(1),
        workspace_id: id(2),
    }
}

fn owner() -> PrincipalRef {
    PrincipalRef {
        authority_id: id(1),
        principal_id: id(3),
    }
}

fn content(index: u128, bytes: &[u8]) -> ArtifactContent {
    ArtifactContent {
        revision: ArtifactRevision {
            schema_version: CoreSchemaVersion,
            reference: ArtifactRef {
                workspace: scope(),
                artifact_id: id(100 + index),
                revision_id: id(200 + index),
                sha256: format!("{:x}", Sha256::digest(bytes)).parse().unwrap(),
            },
            owner: owner(),
            sequence: 1.try_into().unwrap(),
            parent: None,
            kind: "example.document".parse().unwrap(),
            title: "Original artifact title".into(),
            media_type: "application/octet-stream".into(),
            byte_length: bytes.len() as u64,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
        },
        bytes: bytes.to_vec(),
    }
}

#[test]
fn named_empty_local_draft_produces_a_publication_plan_without_normalizing_title() {
    let draft =
        br#"{"format":"cyanrex.task-draft","version":1,"title":"  Empty task  ","payload":[]}"#;
    let plan = TaskDraftPublicationPlan::parse_json(draft)
        .ok()
        .expect("named empty draft must be admitted");
    assert_eq!(plan.title(), "  Empty task  ");
    assert!(plan.items().is_empty());
    let manifest = plan.bind_published(scope(), owner(), &[]).unwrap();
    assert_eq!(manifest.title(), plan.title());
    assert!(manifest.payload().is_empty());
}

#[test]
fn shared_browser_serializations_and_inert_future_language_are_importable() {
    for bytes in [
        include_bytes!("fixtures/task-draft-publication/named-empty.json").as_slice(),
        include_bytes!("fixtures/task-draft-publication/multilingual.json").as_slice(),
        include_bytes!("fixtures/task-draft-publication/future-language.json").as_slice(),
    ] {
        let plan = TaskDraftPublicationPlan::parse_json(bytes)
            .ok()
            .expect("shared fixture");
        let source: Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(plan.title(), source["title"].as_str().unwrap());
        assert_eq!(
            plan.items().len(),
            source["payload"].as_array().unwrap().len()
        );
        for (actual, expected) in plan
            .items()
            .iter()
            .zip(source["payload"].as_array().unwrap())
        {
            assert_eq!(actual.filename(), expected["filename"].as_str().unwrap());
            assert_eq!(actual.language(), expected["language"].as_str().unwrap());
            assert_eq!(actual.text(), expected["text"].as_str().unwrap());
        }
    }
}
