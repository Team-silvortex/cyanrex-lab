use super::*;
use crate::models::collaboration::{VersionedName, WorkspaceRef};
use serde_json::json;

#[test]
fn raw_page_query_accepts_only_canonical_bounded_positions() {
    let id: TaskId = "00000000-0000-0000-0000-000000000003".parse().unwrap();
    let defaults = page_request(None).unwrap_or_else(|_| panic!("absent query is valid"));
    assert_eq!((defaults.after, defaults.limit), (None, None));
    for limit in 1..=32 {
        for query in [
            format!("after={id}&limit={limit}"),
            format!("limit={limit}&after={id}"),
        ] {
            let page = page_request(Some(&query)).unwrap_or_else(|_| panic!("canonical query"));
            assert_eq!((page.after, page.limit), (Some(id), Some(limit)));
        }
    }
    for invalid in [
        "".to_owned(),
        "x".repeat(129),
        "limit=0".to_owned(),
        "limit=33".to_owned(),
        "limit=01".to_owned(),
        "limit=+1".to_owned(),
        "limit=%31".to_owned(),
        "limit=1=2".to_owned(),
        "limit=1&".to_owned(),
        "&limit=1".to_owned(),
        "limit=1&limit=2".to_owned(),
        "limit=1&&limit=2".to_owned(),
        "limit=".to_owned(),
        "limit= 1".to_owned(),
        "%6cimit=1".to_owned(),
        "owner=00000000-0000-0000-0000-000000000001".to_owned(),
        "after=00000000-0000-0000-0000-000000000000".to_owned(),
        "after=00000000000000000000000000000003".to_owned(),
        "after=abcdefab-cdef-abcd-efab-cdefabcdefaB".to_owned(),
        format!("after={id}&after={id}"),
        format!("after={id}%20"),
        format!("after={id}&scope=local"),
    ] {
        assert!(page_request(Some(&invalid)).is_err(), "accepted: {invalid}");
    }
}

#[test]
fn index_projection_keeps_exact_metadata_keys_and_explicit_nulls() {
    let reference = TaskRef {
        workspace: WorkspaceRef {
            authority_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            workspace_id: "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        },
        task_id: "00000000-0000-0000-0000-000000000003".parse().unwrap(),
    };
    let revision = RevisionNumber::try_from(1).unwrap();
    let definition = TaskDefinitionRef {
        package: VersionedName {
            name: "cyanrex.example".parse().unwrap(),
            version: revision,
        },
        name: "cyanrex.notes".parse().unwrap(),
        version: revision,
    };
    for definition_ref in [None, Some(definition)] {
        for next_after in [None, Some(reference)] {
            let page = TaskContentIndexPage {
                entries: vec![TaskContentIndexEntry {
                    reference,
                    revision,
                    title: "<script>inert metadata</script>".into(),
                    status: TaskStatus::Draft,
                    definition_ref: definition_ref.clone(),
                    updated_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
                    payload_items: 32,
                }],
                next_after,
            };
            let wire = serde_json::to_value(IndexResponse::from(page)).unwrap();
            assert_eq!(
                wire,
                json!({
                    "schema_version": 1,
                    "entries": [{
                        "reference": reference,
                        "revision": 1,
                        "title": "<script>inert metadata</script>",
                        "status": "draft",
                        "definition_ref": definition_ref,
                        "updated_at": "1970-01-01T00:00:00Z",
                        "payload_items": 32
                    }],
                    "next_after": next_after
                })
            );
        }
    }
    let empty = serde_json::to_value(IndexResponse::from(TaskContentIndexPage {
        entries: vec![],
        next_after: None,
    }))
    .unwrap();
    assert_eq!(
        empty,
        json!({"schema_version":1,"entries":[],"next_after":null})
    );
}
