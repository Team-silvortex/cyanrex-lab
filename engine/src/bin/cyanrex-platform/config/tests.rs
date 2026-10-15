use super::validate;
use serde_json::{json, Value};

fn config() -> Value {
    json!({
        "format_version": 1,
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
    })
}

fn loopback_config() -> Value {
    let mut value = config();
    value["database"]["transport"] = json!({"kind": "loopback", "address": "127.0.0.1"});
    value
}

fn at_mut<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
    let mut current = value;
    for key in path {
        current = current.get_mut(*key).unwrap();
    }
    current
}

fn valid(value: &Value) {
    assert!(validate(&serde_json::to_vec(value).unwrap()).is_ok());
}

fn rejected(value: &Value) {
    assert_eq!(validate(&serde_json::to_vec(value).unwrap()), Err(()));
}

#[test]
fn platform_config_accepts_complete_explicit_unix_and_literal_loopback_documents() {
    valid(&config());
    for address in ["127.0.0.1", "127.0.0.2", "127.255.255.255", "::1"] {
        let mut value = loopback_config();
        value["database"]["transport"]["address"] = json!(address);
        valid(&value);
    }
    let mut value = config();
    value["database"]["database"] = json!("Platform_DB-1");
    value["database"]["role"] = json!("Runtime_1-test");
    value["database"]["system_identifier"] = json!(u64::MAX);
    value["database"]["database_oid"] = json!(u32::MAX);
    value["database"]["port"] = json!(u16::MAX);
    value["database"]["password"] = json!("");
    valid(&value);
}

#[test]
fn platform_config_requires_all_fields_and_rejects_unknown_fields_at_every_layer() {
    for original in [config(), loopback_config()] {
        for path in [
            vec![],
            vec!["namespaces"],
            vec!["database"],
            vec!["database", "transport"],
            vec!["session_limits"],
        ] {
            let mut inspected = original.clone();
            let keys = at_mut(&mut inspected, &path)
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                let mut value = original.clone();
                at_mut(&mut value, &path)
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
                rejected(&value);
            }
            let mut value = original.clone();
            at_mut(&mut value, &path)
                .as_object_mut()
                .unwrap()
                .insert("unexpected_field".to_owned(), json!(true));
            rejected(&value);
        }
    }
    let mut unix = config();
    unix["database"]["transport"]["address"] = json!("127.0.0.1");
    rejected(&unix);
    let mut loopback = loopback_config();
    loopback["database"]["transport"]["directory"] = json!("/tmp/platform");
    rejected(&loopback);
}

#[test]
fn platform_config_rejects_duplicate_members_even_when_the_values_are_identical() {
    for original in [config(), loopback_config()] {
        for path in [
            vec![],
            vec!["namespaces"],
            vec!["database"],
            vec!["database", "transport"],
            vec!["session_limits"],
        ] {
            let mut inspected = original.clone();
            let object = at_mut(&mut inspected, &path).as_object().unwrap();
            for (key, value) in object {
                let member = format!("{}:{}", serde_json::to_string(key).unwrap(), value);
                let bytes = serde_json::to_string(&original).unwrap();
                assert_eq!(bytes.matches(&member).count(), 1);
                let duplicate = bytes.replacen(&member, &format!("{member},{member}"), 1);
                assert_eq!(validate(duplicate.as_bytes()), Err(()));
            }
        }
    }
}

#[test]
fn platform_config_rejects_wrong_versions_and_non_object_or_wrongly_typed_fields() {
    for version in [
        json!(0),
        json!(2),
        json!(-1),
        json!(1.0),
        json!("1"),
        json!(true),
        Value::Null,
    ] {
        let mut value = config();
        value["format_version"] = version;
        rejected(&value);
    }
    for value in [
        Value::Null,
        json!([]),
        json!(1),
        json!("configuration"),
        json!(true),
    ] {
        rejected(&value);
    }
    for key in [
        "origin",
        "authority_id",
        "workspace_id",
        "namespaces",
        "database",
        "session_limits",
        "artifact_root",
    ] {
        for replacement in [Value::Null, json!([]), json!(false), json!(7)] {
            let mut value = config();
            value[key] = replacement;
            rejected(&value);
        }
    }
}

#[test]
fn platform_config_reuses_canonical_origin_and_nonzero_uuid_constraints() {
    for origin in [
        "https://platform.example",
        "https://platform.example:8443",
        "http://127.0.0.1:43123",
        "http://[::1]:43123",
        "http://localhost:43123",
    ] {
        let mut value = config();
        value["origin"] = json!(origin);
        valid(&value);
    }
    for origin in [
        "",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://user:password@platform.example",
        "http://192.0.2.1:43123",
        "https://platform.example\n",
    ] {
        let mut value = config();
        value["origin"] = json!(origin);
        rejected(&value);
    }
    for key in ["authority_id", "workspace_id"] {
        for id in [
            "",
            "00000000-0000-0000-0000-000000000000",
            "not-a-uuid",
            "00000000-0000-0000-0000-00000000106z",
            " 00000000-0000-0000-0000-000000001069",
        ] {
            let mut value = config();
            value[key] = json!(id);
            rejected(&value);
        }
    }
}

#[test]
fn platform_config_reuses_private_namespace_and_physical_target_constraints() {
    for key in ["source", "task", "artifacts"] {
        for invalid in [
            "",
            "public",
            "information_schema",
            "pg_catalog",
            "pg_private",
            "9source",
            "Source",
            "source-name",
            "source.name",
            "source name",
            "source\0",
            "源",
            &"s".repeat(64),
        ] {
            let mut value = config();
            value["namespaces"][key] = json!(invalid);
            rejected(&value);
        }
    }
    for (left, right) in [
        ("source", "task"),
        ("source", "artifacts"),
        ("task", "artifacts"),
    ] {
        let mut value = config();
        value["namespaces"][right] = value["namespaces"][left].clone();
        rejected(&value);
    }
    for key in ["database", "role"] {
        for invalid in [
            "",
            "name space",
            "name.name",
            "name;DROP",
            "name\0",
            "名",
            &"x".repeat(64),
        ] {
            let mut value = config();
            value["database"][key] = json!(invalid);
            rejected(&value);
        }
    }
    for key in ["system_identifier", "database_oid"] {
        for invalid in [
            json!(0),
            json!(-1),
            json!(1.0),
            json!("1"),
            json!(true),
            Value::Null,
            json!(1e30),
        ] {
            let mut value = config();
            value["database"][key] = invalid;
            rejected(&value);
        }
    }
    let mut value = config();
    value["database"]["database_oid"] = json!(u64::from(u32::MAX) + 1);
    rejected(&value);
    for key in ["source", "task", "artifacts"] {
        let mut value = config();
        value["namespaces"][key] = json!(format!("a{}", "1".repeat(62)));
        valid(&value);
    }
}

#[test]
fn platform_config_reuses_session_stock_limits_without_defaults_or_coercion() {
    for (per_account, total) in [(1, 1), (4, 16), (64, 4096)] {
        let mut value = config();
        value["session_limits"] = json!({"per_account": per_account, "total": total});
        valid(&value);
    }
    for (per_account, total) in [
        (0, 1),
        (1, 0),
        (65, 4096),
        (1, 4097),
        (5, 4),
        (-1, 16),
        (1, -1),
        (65536, 65536),
    ] {
        let mut value = config();
        value["session_limits"] = json!({"per_account": per_account, "total": total});
        rejected(&value);
    }
    for key in ["per_account", "total"] {
        for invalid in [json!(4.0), json!("4"), json!(true), Value::Null] {
            let mut value = config();
            value["session_limits"][key] = invalid;
            rejected(&value);
        }
    }
}

#[test]
fn platform_config_requires_an_explicit_byte_bounded_nul_free_password() {
    for password in [
        String::new(),
        "x".repeat(4096),
        "界".repeat(1365),
        "synthetic\npassword".to_owned(),
    ] {
        let mut value = config();
        value["database"]["password"] = json!(password);
        valid(&value);
    }
    for password in [
        "x".repeat(4097),
        "界".repeat(1366),
        "before\0after".to_owned(),
    ] {
        let mut value = config();
        value["database"]["password"] = json!(password);
        rejected(&value);
    }
    for password in [Value::Null, json!(false), json!(7), json!([])] {
        let mut value = config();
        value["database"]["password"] = password;
        rejected(&value);
    }
}

#[test]
fn platform_config_reuses_local_endpoint_and_complete_socket_byte_budgets() {
    for port in [1u16, 5432, u16::MAX] {
        let directory_bytes = 100 - format!("/.s.PGSQL.{port}").len();
        let directory = format!("/{}", "s".repeat(directory_bytes - 1));
        let mut value = config();
        value["database"]["port"] = json!(port);
        value["database"]["transport"]["directory"] = json!(directory);
        valid(&value);
        value["database"]["transport"]["directory"] = json!(format!("{directory}s"));
        rejected(&value);
    }
    for directory in [
        "",
        ".",
        "relative",
        "/",
        "/tmp/",
        "/tmp//socket",
        "//tmp/socket",
        "/tmp/./socket",
        "/tmp/../socket",
        "/tmp/socket\0",
        "/tmp/socket\n",
        "/tmp/socket\u{85}",
    ] {
        let mut value = config();
        value["database"]["transport"]["directory"] = json!(directory);
        rejected(&value);
    }
    for address in [
        "",
        "localhost",
        "127.0.0.1:5432",
        "0.0.0.0",
        "192.0.2.1",
        "::",
        "::ffff:127.0.0.1",
        "fe80::1",
        "127.0.0.1\0",
    ] {
        let mut value = loopback_config();
        value["database"]["transport"]["address"] = json!(address);
        rejected(&value);
    }
    for (mut value, field) in [(config(), "directory"), (loopback_config(), "address")] {
        for invalid in [Value::Null, json!(false), json!(5432), json!([]), json!({})] {
            value["database"]["transport"][field] = invalid;
            rejected(&value);
        }
    }
    for kind in [
        json!("tcp"),
        json!("Unix"),
        json!(""),
        json!(false),
        Value::Null,
    ] {
        let mut value = config();
        value["database"]["transport"]["kind"] = kind;
        rejected(&value);
    }
    for port in [
        json!(0),
        json!(65536),
        json!(-1),
        json!(5432.0),
        json!("5432"),
        json!(true),
        Value::Null,
    ] {
        let mut value = config();
        value["database"]["port"] = port;
        rejected(&value);
    }
}

#[test]
fn platform_config_artifact_root_is_only_a_canonical_absolute_path_byte_policy() {
    for root in [
        "/synthetic/definitely/nonexistent",
        "/tmp/目录",
        &format!("/{}", "a".repeat(4095)),
        &format!("/{}", "界".repeat(1365)),
    ] {
        let mut value = config();
        value["artifact_root"] = json!(root);
        valid(&value);
    }
    for root in [
        "",
        ".",
        "relative",
        "/",
        "/tmp/",
        "//tmp/platform",
        "/tmp//platform",
        "/tmp/./platform",
        "/tmp/../platform",
        "/tmp/platform/.",
        "/tmp/platform/..",
        "/tmp/platform\0",
        "/tmp/platform\n",
        "/tmp/platform\u{7f}",
        "/tmp/platform\u{85}",
        &format!("/{}", "a".repeat(4096)),
        &format!("/{}", "界".repeat(1366)),
    ] {
        let mut value = config();
        value["artifact_root"] = json!(root);
        rejected(&value);
    }
}

#[test]
fn platform_config_rejects_invalid_utf8_oversized_input_and_trailing_content() {
    let bytes = serde_json::to_vec(&config()).unwrap();
    let mut exact = bytes.clone();
    exact.resize(16 * 1024, b' ');
    assert!(validate(&exact).is_ok());
    exact.push(b' ');
    assert_eq!(validate(&exact), Err(()));
    for suffix in [
        b"false".as_slice(),
        b"{}".as_slice(),
        b"garbage".as_slice(),
        b"\xff".as_slice(),
    ] {
        let mut invalid = bytes.clone();
        invalid.extend_from_slice(suffix);
        assert_eq!(validate(&invalid), Err(()));
    }
    for invalid in [
        b"".as_slice(),
        b"\xff".as_slice(),
        b"\xef\xbb\xbf{}".as_slice(),
        b"{\"format_version\":".as_slice(),
    ] {
        assert_eq!(validate(invalid), Err(()));
    }
    let mut invalid = bytes;
    let position = invalid.iter().position(|byte| *byte == b's').unwrap();
    invalid[position] = 0xff;
    assert_eq!(validate(&invalid), Err(()));
}

#[test]
fn platform_config_rejects_complete_positional_array_records_at_every_layer() {
    let original = config();
    let mut cases = Vec::new();
    for (key, fields) in [
        ("namespaces", vec!["source", "task", "artifacts"]),
        (
            "database",
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
        ("session_limits", vec!["per_account", "total"]),
    ] {
        let mut value = original.clone();
        value[key] = Value::Array(
            fields
                .iter()
                .map(|field| original[key][*field].clone())
                .collect(),
        );
        cases.push(value);
    }
    cases.push(Value::Array(
        [
            "format_version",
            "origin",
            "authority_id",
            "workspace_id",
            "namespaces",
            "database",
            "session_limits",
            "artifact_root",
        ]
        .iter()
        .map(|key| original[*key].clone())
        .collect(),
    ));
    let rejected = cases
        .iter()
        .map(|value| validate(&serde_json::to_vec(value).unwrap()).is_err())
        .collect::<Vec<_>>();
    assert!(
        rejected.iter().all(|value| *value),
        "all named records must be JSON objects"
    );
}
