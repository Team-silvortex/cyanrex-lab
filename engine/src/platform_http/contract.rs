use super::errors::HttpError;
use crate::{
    models::collaboration::{
        ArtifactRef, CoreSchemaVersion, RevisionNumber, TaskContentManifest, TaskId, TaskSnapshot,
        TaskStatus,
    },
    services::{auth_service::durable_source::SessionTaskContent, task_store::TaskContentSnapshot},
};
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::{fmt, marker::PhantomData};

// Preserve map field occurrences all the way into strict DTO/model deserializers. Going through
// serde_json::Value would erase duplicate keys; ordinary derive also admits positional arrays.
struct MapOnly<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RecordVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordVisitor<T> {
            type Value = MapOnly<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object record")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }
        deserializer.deserialize_map(RecordVisitor(PhantomData))
    }
}

pub(super) fn parse<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, HttpError> {
    serde_json::from_slice::<MapOnly<T>>(bytes)
        .map(|value| value.0)
        .map_err(|_| HttpError::invalid_request())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    pub schema_version: CoreSchemaVersion,
    pub task_id: TaskId,
    pub manifest: TaskContentManifest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Replace {
    pub schema_version: CoreSchemaVersion,
    pub expected_revision: RevisionNumber,
    pub manifest: TaskContentManifest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Transition {
    pub schema_version: CoreSchemaVersion,
    pub expected_revision: RevisionNumber,
    // Unit enums accept object representations in serde; the HTTP contract admits strings only.
    pub status: String,
}
impl Transition {
    pub(super) fn status(&self) -> Result<TaskStatus, HttpError> {
        Ok(match self.status.as_str() {
            "draft" => TaskStatus::Draft,
            "ready" => TaskStatus::Ready,
            "in_progress" => TaskStatus::InProgress,
            "blocked" => TaskStatus::Blocked,
            "in_review" => TaskStatus::InReview,
            "cancelled" => TaskStatus::Cancelled,
            _ => return Err(HttpError::invalid_request()),
        })
    }
}

#[derive(Serialize)]
pub(super) struct SnapshotResponse {
    schema_version: u8,
    task: TaskSnapshot,
    manifest: TaskContentManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    contents: Option<Vec<TextResponse>>,
}
#[derive(Serialize)]
struct TextResponse {
    artifact: ArtifactRef,
    text: String,
}

impl From<TaskContentSnapshot> for SnapshotResponse {
    fn from(snapshot: TaskContentSnapshot) -> Self {
        Self {
            schema_version: 1,
            task: snapshot.task,
            manifest: snapshot.manifest,
            contents: None,
        }
    }
}
impl TryFrom<SessionTaskContent> for SnapshotResponse {
    type Error = HttpError;
    fn try_from(value: SessionTaskContent) -> Result<Self, Self::Error> {
        let mut result = Self::from(value.snapshot);
        result.contents = Some(
            value
                .contents
                .into_iter()
                .map(|content| {
                    Ok(TextResponse {
                        artifact: content.revision.reference,
                        text: String::from_utf8(content.bytes).map_err(|_| {
                            HttpError::dispatched(
                                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                "service_unavailable",
                            )
                        })?,
                    })
                })
                .collect::<Result<_, Self::Error>>()?,
        );
        Ok(result)
    }
}
