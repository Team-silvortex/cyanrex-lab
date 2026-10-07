use super::{PlannedTextPublication, TaskDraftPublicationPlan, MAX_TASK_DRAFT_BYTES};
use crate::models::collaboration::{
    validate_task_content_title, validate_text_payload_metadata, ContractError, RevisionNumber,
    MAX_TASK_CONTENT_ITEMS, MAX_TEXT_PAYLOAD_BYTES,
};
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{collections::HashSet, fmt, marker::PhantomData};

// Forward maps directly to strict field deserializers; never erase duplicates through Value.
struct MapOnly<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RecordVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordVisitor<T> {
            type Value = MapOnly<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object record")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }
        deserializer.deserialize_map(RecordVisitor(PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDraft {
    format: String,
    version: u8,
    title: String,
    payload: Vec<MapOnly<RawText>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawText {
    id: String,
    revision: u64,
    kind: String,
    filename: String,
    language: String,
    text: String,
}

pub(super) fn parse(bytes: &[u8]) -> Result<TaskDraftPublicationPlan, ContractError> {
    if bytes.len() > MAX_TASK_DRAFT_BYTES {
        return Err(ContractError("task draft exceeds its byte limit"));
    }
    let MapOnly(raw): MapOnly<RawDraft> =
        serde_json::from_slice(bytes).map_err(|_| ContractError("invalid portable task draft"))?;
    if raw.format != "cyanrex.task-draft" || raw.version != 1 {
        return Err(ContractError("unsupported portable task draft format"));
    }
    validate_task_content_title(&raw.title)?;
    if raw.payload.len() > MAX_TASK_CONTENT_ITEMS {
        return Err(ContractError("task draft has too many payload items"));
    }
    let mut seen = HashSet::with_capacity(raw.payload.len());
    let mut items = Vec::with_capacity(raw.payload.len());
    for MapOnly(item) in raw.payload {
        if !(1..=80).contains(&item.id.len())
            || !item
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || !seen.insert(item.id)
        {
            return Err(ContractError(
                "task draft local IDs must be bounded unique labels",
            ));
        }
        // This checks the shared integer range only; no local revision enters the plan or manifest.
        RevisionNumber::try_from(item.revision)?;
        if item.kind != "text" {
            return Err(ContractError("unsupported task draft payload kind"));
        }
        validate_text_payload_metadata(&item.filename, &item.language)?;
        if item.text.len() > MAX_TEXT_PAYLOAD_BYTES
            || item
                .text
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\t' | '\n' | '\r'))
        {
            return Err(ContractError("task draft text must be bounded valid text"));
        }
        items.push(PlannedTextPublication {
            filename: item.filename,
            language: item.language,
            text: item.text,
        });
    }
    Ok(TaskDraftPublicationPlan {
        title: raw.title,
        items,
    })
}
