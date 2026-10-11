use super::*;

fn valid() -> String {
    create_body(
        TASK.parse().unwrap(),
        &values()[0].reference,
        &manifest("Frozen reference", vec![]),
    )
}

#[tokio::test]
async fn catalog_http_definition_reference_and_package_require_exact_objects_and_fields() {
    let f = Offline::new().await;
    let parsed: Value = serde_json::from_str(&valid()).unwrap();
    let reference = parsed["definition_ref"].clone();
    let mut cases = vec![
        json!(null),
        json!([]),
        json!("sample.document.prepare"),
        json!([
            reference["package"],
            reference["name"],
            reference["version"]
        ]),
    ];
    for key in ["package", "name", "version"] {
        let mut value = reference.clone();
        value.as_object_mut().unwrap().remove(key);
        cases.push(value);
    }
    for (key, value) in [
        ("definition", json!({})),
        ("catalog", json!("sample.document")),
        ("title", json!("Client override")),
        ("evidence_schema", json!(null)),
        ("assessment_policy", json!(null)),
        ("namespace", json!("private_tasks")),
    ] {
        let mut candidate = reference.clone();
        candidate[key] = value;
        cases.push(candidate);
    }
    for package in [
        json!(null),
        json!([]),
        json!("sample.document"),
        json!(["sample.document", 1]),
        json!({"name":"sample.document"}),
        json!({"version":1}),
        json!({"name":"sample.document","version":1,"title":"Untrusted"}),
    ] {
        let mut candidate = reference.clone();
        candidate["package"] = package;
        cases.push(candidate);
    }
    for invalid in [
        json!(0),
        json!(-1),
        json!(1.0),
        json!("1"),
        json!(null),
        json!(true),
        json!(9007199254740992_u64),
    ] {
        for package in [false, true] {
            let mut candidate = reference.clone();
            if package {
                candidate["package"]["version"] = invalid.clone();
            } else {
                candidate["version"] = invalid.clone();
            }
            cases.push(candidate);
        }
    }
    for name in [
        "",
        "Sample.document",
        "sample..document",
        "文档.sample",
        "sample.\ntext",
        &"x".repeat(257),
    ] {
        for package in [false, true] {
            let mut candidate = reference.clone();
            if package {
                candidate["package"]["name"] = json!(name);
            } else {
                candidate["name"] = json!(name);
            }
            cases.push(candidate);
        }
    }
    for body_ref in cases {
        let mut body = parsed.clone();
        body["definition_ref"] = body_ref;
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body.to_string()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for key in ["schema_version", "definition_ref", "manifest", "task_id"] {
        let mut body = parsed.clone();
        body.as_object_mut().unwrap().remove(key);
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body.to_string()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for key in [
        "definition",
        "owner",
        "workspace",
        "catalog",
        "provider",
        "assessment",
    ] {
        let mut body = parsed.clone();
        body[key] = json!(null);
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body.to_string()),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
    for definition in values() {
        error(
            f.app.clone(),
            request(
                "POST",
                COLLECTION,
                TOKEN,
                create_body(
                    TASK.parse().unwrap(),
                    &definition.reference,
                    &manifest("Valid bound or unbound reference", vec![]),
                ),
            ),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        )
        .await;
    }
}

#[tokio::test]
async fn catalog_http_duplicate_reference_fields_are_rejected_in_the_original_json_stream() {
    let f = Offline::new().await;
    let valid = valid();
    let parsed: Value = serde_json::from_str(&valid).unwrap();
    let reference = parsed["definition_ref"].to_string();
    let package = parsed["definition_ref"]["package"].to_string();
    let mut cases = vec![valid.replacen(
        "\"definition_ref\":",
        &format!("\"definition_ref\":{reference},\"definition_ref\":"),
        1,
    )];
    for (container, field, replacement) in [
        (reference.clone(), "name", "\"sample.memo.prepare\""),
        (reference.clone(), "version", "2"),
        (reference.clone(), "package", "null"),
        (package.clone(), "name", "\"sample.memo\""),
        (package.clone(), "version", "2"),
    ] {
        for added in [
            replacement.to_string(),
            serde_json::from_str::<Value>(&container).unwrap()[field].to_string(),
        ] {
            let duplicate = format!("{{\"{field}\":{added},{}", &container[1..]);
            cases.push(valid.replacen(&container, &duplicate, 1));
        }
    }
    let escaped = reference.replacen(
        "\"name\":",
        "\"n\\u0061me\":\"sample.memo.prepare\",\"name\":",
        1,
    );
    cases.push(valid.replacen(&reference, &escaped, 1));
    for body in cases {
        assert_ne!(body, valid, "each wire injection must be effective");
        offline_error(
            &f,
            request("POST", COLLECTION, TOKEN, body),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        )
        .await;
    }
}

#[tokio::test]
async fn catalog_http_and_manual_http_are_disjoint_explicit_modes_without_route_or_body_fallback() {
    let f = Offline::new().await;
    let manual = manual_http_fixture::Offline::new().await;
    for path in [
        format!("{COLLECTION}/"),
        format!("{COLLECTION}/{TASK}/unknown"),
        format!("{COLLECTION}/{TASK}/content/extra"),
    ] {
        offline_error(
            &f,
            request("GET", &path, TOKEN, Body::empty()),
            StatusCode::NOT_FOUND,
            "not_found",
        )
        .await;
    }
    for (app, path, body) in [
        (f.app.clone(), manual_http_fixture::COLLECTION, valid()),
        (manual.app.clone(), COLLECTION, valid()),
    ] {
        error(
            app,
            request("POST", path, TOKEN, body),
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        )
        .await;
    }
    error(
        manual.app.clone(),
        request("POST", manual_http_fixture::COLLECTION, TOKEN, valid()),
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    )
    .await;
    error(
        manual.app.clone(),
        request(
            "POST",
            manual_http_fixture::COLLECTION,
            TOKEN,
            manual_http_fixture::create_body(
                TASK.parse().unwrap(),
                &manifest("Manual still works", vec![]),
            ),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
