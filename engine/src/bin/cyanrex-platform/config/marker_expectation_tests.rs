use super::validate;
use serde_json::{json, Value};

fn config(version: u8) -> Value {
    let mut value = json!({
        "format_version": version,
        "origin": "https://platform.example:8443",
        "authority_id": "00000000-0000-0000-0000-000000001069",
        "workspace_id": "00000000-0000-0000-0000-00000000106a",
        "namespaces": {"source": "source_private", "task": "task_private", "artifacts": "artifacts_private"},
        "database": {"database": "platform_db", "role": "platform_runtime", "system_identifier": 123,
            "database_oid": 456, "transport": {"kind": "unix", "directory": "/tmp/platform"},
            "port": 5432, "password": "synthetic_explicit_password"},
        "session_limits": {"per_account": 4, "total": 16},
        "artifact_root": "/synthetic/nonexistent/platform/artifacts"
    });
    if version >= 2 {
        value["catalog_identity"] = json!({"role_oid": 101, "database_owner_oid": 102,
            "source": {"oid": 201, "owner_oid": 102}, "task": {"oid": 202, "owner_oid": 102},
            "artifacts": {"oid": 203, "owner_oid": 102}});
    }
    if version == 3 {
        value["marker_identity"] = json!({
            "source": {"oid": 301, "owner_oid": 102}, "identity": {"oid": 302, "owner_oid": 102},
            "access": {"oid": 303, "owner_oid": 102}, "task": {"oid": 304, "owner_oid": 102},
            "artifacts": {"oid": 305, "owner_oid": 102}});
    }
    value
}

fn valid(value: &Value) {
    assert!(validate(&serde_json::to_vec(value).unwrap()).is_ok());
}
fn rejected(value: &Value) {
    assert_eq!(validate(&serde_json::to_vec(value).unwrap()), Err(()));
}
fn record_paths() -> Vec<&'static str> {
    vec![
        "",
        "/namespaces",
        "/database",
        "/database/transport",
        "/session_limits",
        "/catalog_identity",
        "/catalog_identity/source",
        "/catalog_identity/task",
        "/catalog_identity/artifacts",
        "/marker_identity",
        "/marker_identity/source",
        "/marker_identity/identity",
        "/marker_identity/access",
        "/marker_identity/task",
        "/marker_identity/artifacts",
    ]
}
const MARKERS: [&str; 5] = ["source", "identity", "access", "task", "artifacts"];

#[test]
fn platform_config_marker_expectation_accepts_complete_version_three_without_changing_old_formats()
{
    valid(&config(1));
    valid(&config(2));
    valid(&config(3));
    let mut value = config(3);
    value["database"]["transport"] = json!({"kind": "loopback", "address": "::1"});
    value["database"]["password"] = json!("");
    valid(&value);
    // pg_class OIDs may numerically overlap other catalogs; owners need not own the namespace.
    for (index, (oid, owner)) in [101u32, 102, 201, 202, u32::MAX]
        .into_iter()
        .zip([102u32, 202, 203, u32::MAX, 456])
        .enumerate()
    {
        value["marker_identity"][MARKERS[index]] = json!({"oid": oid, "owner_oid": owner});
    }
    valid(&value);
}

#[test]
fn platform_config_marker_expectation_requires_every_field_and_rejects_unknown_nested_members() {
    let original = config(3);
    for path in record_paths() {
        for key in original.pointer(path).unwrap().as_object().unwrap().keys() {
            let mut value = original.clone();
            value
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            rejected(&value);
        }
        let mut value = original.clone();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".to_owned(), json!(true));
        rejected(&value);
    }
}

#[test]
fn platform_config_marker_expectation_rejects_identical_duplicate_keys_in_every_record() {
    let original = config(3);
    let document = serde_json::to_string(&original).unwrap();
    for path in record_paths() {
        let record = original.pointer(path).unwrap();
        let compact = serde_json::to_string(record).unwrap();
        assert_eq!(document.matches(&compact).count(), 1);
        for (key, value) in record.as_object().unwrap() {
            let member = format!("{}:{}", serde_json::to_string(key).unwrap(), value);
            assert_eq!(compact.matches(&member).count(), 1);
            let changed = compact.replacen(&member, &format!("{member},{member}"), 1);
            assert_eq!(
                validate(document.replacen(&compact, &changed, 1).as_bytes()),
                Err(())
            );
        }
    }
}

#[test]
fn platform_config_marker_expectation_requires_named_objects_not_complete_positional_arrays() {
    let original = config(3);
    for (path, fields) in [
        (
            "",
            vec![
                "format_version",
                "origin",
                "authority_id",
                "workspace_id",
                "namespaces",
                "database",
                "session_limits",
                "artifact_root",
                "catalog_identity",
                "marker_identity",
            ],
        ),
        ("/namespaces", vec!["source", "task", "artifacts"]),
        (
            "/database",
            vec![
                "database",
                "role",
                "system_identifier",
                "database_oid",
                "transport",
                "port",
                "password",
            ],
        ),
        ("/database/transport", vec!["kind", "directory"]),
        ("/session_limits", vec!["per_account", "total"]),
        (
            "/catalog_identity",
            vec![
                "role_oid",
                "database_owner_oid",
                "source",
                "task",
                "artifacts",
            ],
        ),
        ("/catalog_identity/source", vec!["oid", "owner_oid"]),
        ("/catalog_identity/task", vec!["oid", "owner_oid"]),
        ("/catalog_identity/artifacts", vec!["oid", "owner_oid"]),
        (
            "/marker_identity",
            vec!["source", "identity", "access", "task", "artifacts"],
        ),
        ("/marker_identity/source", vec!["oid", "owner_oid"]),
        ("/marker_identity/identity", vec!["oid", "owner_oid"]),
        ("/marker_identity/access", vec!["oid", "owner_oid"]),
        ("/marker_identity/task", vec!["oid", "owner_oid"]),
        ("/marker_identity/artifacts", vec!["oid", "owner_oid"]),
    ] {
        let record = original.pointer(path).unwrap();
        let positional = Value::Array(fields.iter().map(|field| record[*field].clone()).collect());
        for wrong in [
            positional,
            Value::Null,
            json!(false),
            json!(7),
            json!("record"),
        ] {
            let mut value = original.clone();
            *value.pointer_mut(path).unwrap() = wrong;
            rejected(&value);
        }
    }
}

#[test]
fn platform_config_marker_expectation_rejects_each_zero_out_of_range_or_wrongly_typed_oid() {
    for marker in MARKERS {
        for field in ["oid", "owner_oid"] {
            for invalid in [
                json!(0),
                json!(-1),
                json!(1.0),
                json!(u64::from(u32::MAX) + 1),
                json!(u64::MAX),
                json!("101"),
                json!(true),
                Value::Null,
                json!([]),
                json!({}),
            ] {
                let mut value = config(3);
                value["marker_identity"][marker][field] = invalid;
                rejected(&value);
            }
        }
    }
}

#[test]
fn platform_config_marker_expectation_rejects_all_marker_aliases_and_runtime_role_ownership() {
    for left in 0..MARKERS.len() {
        for right in left + 1..MARKERS.len() {
            let mut value = config(3);
            value["marker_identity"][MARKERS[right]]["oid"] =
                value["marker_identity"][MARKERS[left]]["oid"].clone();
            rejected(&value);
        }
        let mut value = config(3);
        value["marker_identity"][MARKERS[left]]["owner_oid"] = json!(101);
        rejected(&value);
    }
}

#[test]
fn platform_config_marker_expectation_legacy_formats_forbid_marker_declarations_including_null() {
    for version in [1, 2] {
        for markers in [
            config(3)["marker_identity"].clone(),
            Value::Null,
            json!({}),
            json!([]),
            json!(false),
            json!("markers"),
        ] {
            let mut value = config(version);
            value["marker_identity"] = markers;
            rejected(&value);
        }
    }
    for field in ["catalog_identity", "marker_identity"] {
        let mut missing = config(3);
        missing.as_object_mut().unwrap().remove(field);
        rejected(&missing);
        let mut null = config(3);
        null[field] = Value::Null;
        rejected(&null);
    }
}

#[test]
fn platform_config_marker_expectation_preserves_original_validators_and_exact_version_types() {
    for version in [
        json!(0),
        json!(4),
        json!(-1),
        json!(3.0),
        json!("3"),
        json!(true),
        Value::Null,
    ] {
        let mut value = config(3);
        value["format_version"] = version;
        rejected(&value);
    }
    for (path, wrong) in [
        ("/origin", json!("http://192.0.2.1")),
        ("/namespaces/task", json!("source_private")),
        ("/database/port", json!(0)),
        ("/database/password", json!("before\0after")),
        ("/session_limits/per_account", json!(0)),
        ("/artifact_root", json!("relative")),
        ("/catalog_identity/role_oid", json!(0)),
        ("/catalog_identity/source/oid", json!(202)),
    ] {
        let mut value = config(3);
        *value.pointer_mut(path).unwrap() = wrong;
        rejected(&value);
    }
}

#[test]
fn platform_config_marker_expectation_keeps_the_sixteen_kibibyte_and_utf8_document_boundaries() {
    for version in [1, 2, 3] {
        let mut bytes = serde_json::to_vec(&config(version)).unwrap();
        bytes.resize(16 * 1024, b' ');
        assert!(validate(&bytes).is_ok());
        bytes.push(b' ');
        assert_eq!(validate(&bytes), Err(()));
    }
    let bytes = serde_json::to_vec(&config(3)).unwrap();
    for suffix in [b"{}".as_slice(), b"false".as_slice(), b"\xff".as_slice()] {
        let mut invalid = bytes.clone();
        invalid.extend_from_slice(suffix);
        assert_eq!(validate(&invalid), Err(()));
    }
}
