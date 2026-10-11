use crate::{
    models::collaboration::{RevisionNumber, TaskDefinitionRef, TaskId, TaskRef, TaskStatus},
    platform_http::errors::HttpError,
    services::task_store::{TaskContentIndexEntry, TaskContentIndexPage},
};
use chrono::{DateTime, Utc};
use serde::Serialize;

pub(super) struct PageRequest {
    pub after: Option<TaskId>,
    pub limit: Option<usize>,
}

/// Parse raw URI bytes only. No form decoding, normalization or authority-bearing fields.
pub(super) fn page_request(query: Option<&str>) -> Result<PageRequest, HttpError> {
    let mut page = PageRequest {
        after: None,
        limit: None,
    };
    let Some(query) = query else {
        return Ok(page);
    };
    if query.is_empty() || query.len() > 128 {
        return Err(HttpError::invalid_request());
    }
    for pair in query.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(HttpError::invalid_request)?;
        match name {
            "after" if page.after.is_none() => {
                page.after = Some(value.parse().map_err(|_| HttpError::invalid_request())?);
            }
            "limit" if page.limit.is_none() => {
                if value.is_empty()
                    || value.starts_with('0')
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(HttpError::invalid_request());
                }
                let limit = value.parse().map_err(|_| HttpError::invalid_request())?;
                if !(1..=32).contains(&limit) {
                    return Err(HttpError::invalid_request());
                }
                page.limit = Some(limit);
            }
            _ => return Err(HttpError::invalid_request()),
        }
    }
    Ok(page)
}

/// Private projection only: service values are not wire DTOs or decoded request authority.
#[derive(Serialize)]
pub(super) struct IndexResponse {
    schema_version: u8,
    entries: Vec<IndexEntry>,
    // Null remains explicit, including on an empty or exhausted page.
    next_after: Option<TaskRef>,
}

#[derive(Serialize)]
struct IndexEntry {
    reference: TaskRef,
    revision: RevisionNumber,
    title: String,
    status: TaskStatus,
    definition_ref: Option<TaskDefinitionRef>,
    updated_at: DateTime<Utc>,
    payload_items: usize,
}

impl From<TaskContentIndexEntry> for IndexEntry {
    fn from(entry: TaskContentIndexEntry) -> Self {
        Self {
            reference: entry.reference,
            revision: entry.revision,
            title: entry.title,
            status: entry.status,
            definition_ref: entry.definition_ref,
            updated_at: entry.updated_at,
            payload_items: entry.payload_items,
        }
    }
}

impl From<TaskContentIndexPage> for IndexResponse {
    fn from(page: TaskContentIndexPage) -> Self {
        Self {
            schema_version: 1,
            entries: page.entries.into_iter().map(IndexEntry::from).collect(),
            next_after: page.next_after,
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
