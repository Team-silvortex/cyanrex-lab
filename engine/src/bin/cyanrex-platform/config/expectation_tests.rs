use super::validate;
use serde_json::{json, Value};

fn config(version: u8) -> Value {
    let mut value = json!({
        "format_version": version,
        "origin": "https://platform.example:8443",
        "authority_id": "00000000-0000-0000-0000-000000001069",
        "workspace_id": "00000000-0000-0000-0000-00000000106a",
        "namespaces": {"source": "source_private", "task": "task_private", "artifacts": "artifacts_private"},
        "database": {
            "database": "platform_db", "role": "platform_runtime",
            "system_identifier": 123, "database_oid": 456,
            "transport": {"kind": "unix", "directory": "/tmp/platform"},
            "port": 5432, "password": "synthetic_explicit_password"
        },
        "session_limits": {"per_account": 4, "total": 16},
        "artifact_root": "/synthetic/nonexistent/platform/artifacts"
    });
    if version == 2 {
        value["catalog_identity"] = json!({
            "role_oid": 101, "database_owner_oid": 102,
            "source": {"oid": 201, "owner_oid": 102},
            "task": {"oid": 202, "owner_oid": 102},
            "artifacts": {"oid": 203, "owner_oid": 102}
        });
    }
    value
}

fn valid(value: &Value) {
    assert!(validate(&serde_json::to_vec(value).unwrap()).is_ok());
}

fn rejected(value: &Value) {
    assert_eq!(validate(&serde_json::to_vec(value).unwrap()), Err(()));
}

fn paths() -> Vec<&'static str> {
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
    ]
}

fn oid_paths() -> Vec<&'static str> {
    vec![
        "/catalog_identity/role_oid",
        "/catalog_identity/database_owner_oid",
        "/catalog_identity/source/oid",
        "/catalog_identity/source/owner_oid",
        "/catalog_identity/task/oid",
        "/catalog_identity/task/owner_oid",
        "/catalog_identity/artifacts/oid",
        "/catalog_identity/artifacts/owner_oid",
    ]
}

#[test]
fn platform_config_expectation_accepts_version_two_and_preserves_complete_version_one() {
    valid(&config(1));
    valid(&config(2));
    let mut value = config(2);
    value["database"]["transport"] = json!({"kind": "loopback", "address": "::1"});
    valid(&value);
    value["catalog_identity"] = json!({
        "role_oid": 7, "database_owner_oid": 8,
        "source": {"oid": 7, "owner_oid": 8},
        "task": {"oid": 8, "owner_oid": 8},
        "artifacts": {"oid": 456, "owner_oid": 8}
    });
    valid(&value);
    value["catalog_identity"] = json!({
        "role_oid": u32::MAX, "database_owner_oid": 1,
        "source": {"oid": u32::MAX, "owner_oid": 1},
        "task": {"oid": 1, "owner_oid": 2},
        "artifacts": {"oid": 456, "owner_oid": 3}
    });
    valid(&value);
}

#[test]
fn platform_config_expectation_requires_every_version_two_field_and_rejects_nested_unknowns() {
    let original = config(2);
    for path in paths() {
        let object = original.pointer(path).unwrap().as_object().unwrap();
        for key in object.keys() {
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
fn platform_config_expectation_rejects_same_value_duplicate_keys_in_every_record() {
    let original = config(2);
    let document = serde_json::to_string(&original).unwrap();
    for path in paths() {
        let object = original.pointer(path).unwrap();
        let compact = serde_json::to_string(object).unwrap();
        assert_eq!(document.matches(&compact).count(), 1);
        for (key, value) in object.as_object().unwrap() {
            let member = format!("{}:{}", serde_json::to_string(key).unwrap(), value);
            assert_eq!(compact.matches(&member).count(), 1);
            let replacement = compact.replacen(&member, &format!("{member},{member}"), 1);
            let duplicate = document.replacen(&compact, &replacement, 1);
            assert_eq!(validate(duplicate.as_bytes()), Err(()));
        }
    }
}

#[test]
fn platform_config_expectation_rejects_positional_arrays_and_wrong_container_types() {
    let original = config(2);
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
    ] {
        let object = original.pointer(path).unwrap();
        let positional = Value::Array(fields.iter().map(|field| object[*field].clone()).collect());
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
fn platform_config_expectation_rejects_missing_zero_out_of_range_and_wrongly_typed_oids() {
    for path in oid_paths() {
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
            let mut value = config(2);
            *value.pointer_mut(path).unwrap() = invalid;
            rejected(&value);
        }
    }
}

#[test]
fn platform_config_expectation_rejects_namespace_aliases_and_runtime_owned_objects() {
    for (left, right) in [
        ("source", "task"),
        ("source", "artifacts"),
        ("task", "artifacts"),
    ] {
        let mut value = config(2);
        value["catalog_identity"][right]["oid"] = value["catalog_identity"][left]["oid"].clone();
        rejected(&value);
    }
    for path in [
        "/catalog_identity/database_owner_oid",
        "/catalog_identity/source/owner_oid",
        "/catalog_identity/task/owner_oid",
        "/catalog_identity/artifacts/owner_oid",
    ] {
        let mut value = config(2);
        *value.pointer_mut(path).unwrap() = json!(101);
        rejected(&value);
    }
}

#[test]
fn platform_config_expectation_version_one_forbids_any_catalog_identity_including_null() {
    for identity in [
        config(2)["catalog_identity"].clone(),
        Value::Null,
        json!({}),
        json!([]),
        json!(false),
        json!("identity"),
    ] {
        let mut value = config(1);
        value["catalog_identity"] = identity;
        rejected(&value);
    }
}

#[test]
fn platform_config_expectation_version_two_requires_identity_and_exact_version_type() {
    let mut missing = config(2);
    missing.as_object_mut().unwrap().remove("catalog_identity");
    rejected(&missing);
    for version in [
        json!(0),
        json!(3),
        json!(-1),
        json!(2.0),
        json!("2"),
        json!(true),
        Value::Null,
    ] {
        let mut value = config(2);
        value["format_version"] = version;
        rejected(&value);
    }
}

#[test]
fn platform_config_expectation_preserves_the_exact_sixteen_kibibyte_input_limit() {
    for version in [1, 2] {
        let mut bytes = serde_json::to_vec(&config(version)).unwrap();
        bytes.resize(16 * 1024, b' ');
        assert!(validate(&bytes).is_ok());
        bytes.push(b' ');
        assert_eq!(validate(&bytes), Err(()));
    }
    let bytes = serde_json::to_vec(&config(2)).unwrap();
    for suffix in [b"{}".as_slice(), b"false".as_slice(), b"\xff".as_slice()] {
        let mut invalid = bytes.clone();
        invalid.extend_from_slice(suffix);
        assert_eq!(validate(&invalid), Err(()));
    }
}
