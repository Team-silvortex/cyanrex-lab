use super::{parse, MapOnly, Presence, StorageExpectation};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(default)]
    identity: Presence<Record>,
}

#[test]
fn platform_config_presence_needs_no_record_default_and_preserves_missing_records() {
    assert!(matches!(Presence::<Record>::default(), Presence::Absent));
    let missing: MapOnly<Envelope> = serde_json::from_slice(b"{}").unwrap();
    assert!(matches!(missing.0.identity, Presence::Absent));
    let supplied: MapOnly<Envelope> =
        serde_json::from_slice(br#"{"identity":{"count":7}}"#).unwrap();
    assert!(matches!(
        supplied.0.identity,
        Presence::Present(MapOnly(Record { count: 7 }))
    ));
}

#[test]
fn platform_config_presence_never_turns_null_arrays_unknown_or_duplicate_keys_into_absence() {
    for bytes in [
        br#"{"identity":null}"#.as_slice(),
        br#"{"identity":[]}"#,
        br#"{"identity":[7]}"#,
        br#"{"identity":false}"#,
        br#"{"identity":7}"#,
        br#"{"identity":{}}"#,
        br#"{"identity":{"count":7,"extra":7}}"#,
        br#"{"identity":{"count":7,"count":7}}"#,
        br#"{"identity":{"count":7},"identity":{"count":7}}"#,
    ] {
        assert!(serde_json::from_slice::<MapOnly<Envelope>>(bytes).is_err());
    }
}

#[test]
fn platform_config_validated_storage_has_one_exhaustive_mode_per_unchanged_format() {
    let base = json!({
        "format_version": 1,
        "origin":"https://platform.example:8443",
        "authority_id":"00000000-0000-0000-0000-000000001069",
        "workspace_id":"00000000-0000-0000-0000-00000000106a",
        "namespaces":{"source":"source_private","task":"task_private","artifacts":"artifacts_private"},
        "database":{"database":"platform_db","role":"platform_runtime","system_identifier":123,
            "database_oid":456,"transport":{"kind":"unix","directory":"/tmp/platform"},
            "port":5432,"password":"synthetic_explicit_password"},
        "session_limits":{"per_account":4,"total":16},
        "artifact_root":"/synthetic/nonexistent/platform/artifacts"
    });
    let catalog = json!({"role_oid":101,"database_owner_oid":102,
        "source":{"oid":201,"owner_oid":102},"task":{"oid":202,"owner_oid":102},
        "artifacts":{"oid":203,"owner_oid":102}});
    let markers = json!({"source":{"oid":301,"owner_oid":102},"identity":{"oid":302,"owner_oid":102},
        "access":{"oid":303,"owner_oid":102},"task":{"oid":304,"owner_oid":102},
        "artifacts":{"oid":305,"owner_oid":102}});
    for version in 1..=3 {
        let mut value = base.clone();
        value["format_version"] = json!(version);
        if version >= 2 {
            value["catalog_identity"] = catalog.clone();
        }
        if version == 3 {
            value["marker_identity"] = markers.clone();
        }
        let validated = parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let selected = match validated.storage_expectation {
            StorageExpectation::Current => 1,
            StorageExpectation::Catalog(_) => 2,
            StorageExpectation::Markers(_) => 3,
        };
        assert_eq!(selected, version);
    }
}
