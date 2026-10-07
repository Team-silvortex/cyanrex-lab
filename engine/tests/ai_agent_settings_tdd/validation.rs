use super::*;
use serde_json::{json, Value};

#[tokio::test]
async fn validation_rejects_invalid_metadata_without_creating_storage() {
    let fixture = Fixture::new();
    let baseline = serde_json::to_value(update(0)).unwrap();
    let mut cases = Vec::new();
    for (field, values) in [
        (
            "id",
            vec![json!(""), json!("A"), json!("a.b"), json!("a".repeat(65))],
        ),
        (
            "name",
            vec![
                json!(""),
                json!("\u{feff}"),
                json!(" \u{feff} "),
                json!("  \t"),
                json!("x\ny"),
                json!("x".repeat(129)),
            ],
        ),
        (
            "model",
            vec![
                json!(""),
                json!("\u{feff}"),
                json!(" \u{feff} "),
                json!("\0"),
                json!("x".repeat(129)),
            ],
        ),
        (
            "credential_ref",
            vec![
                json!(""),
                json!("lower"),
                json!("KEY-VALUE"),
                json!("A".repeat(65)),
            ],
        ),
        (
            "base_url",
            vec![
                json!("relative"),
                json!("http://remote.example/v1"),
                json!("file:///tmp/x"),
                json!("https://user:pass@example.test"),
                json!("https://@example.test"),
                json!("https://example.test/?key=secret"),
                json!("https://example.test/#x"),
                json!("https:///"),
                json!("https://example.test/\n"),
            ],
        ),
    ] {
        for value in values {
            let mut candidate = baseline.clone();
            candidate["profiles"][0][field] = value;
            cases.push(candidate);
        }
    }
    let mut missing = baseline.clone();
    missing["default_profile_id"] = json!("missing");
    cases.push(missing);
    let mut disabled = baseline.clone();
    disabled["profiles"][0]["enabled"] = json!(false);
    cases.push(disabled);
    let mut duplicate = baseline.clone();
    duplicate["profiles"] = json!([baseline["profiles"][0], baseline["profiles"][0]]);
    cases.push(duplicate);
    let mut too_many = baseline.clone();
    too_many["profiles"] = Value::Array(
        (0..17)
            .map(|index| {
                let mut profile = baseline["profiles"][0].clone();
                profile["id"] = json!(format!("agent_{index}"));
                profile
            })
            .collect(),
    );
    cases.push(too_many);
    for (index, value) in cases.into_iter().enumerate() {
        let request = serde_json::from_value(value).unwrap();
        assert_eq!(
            fixture.store().update(request).await,
            Err(AiAgentSettingsError::InvalidInput),
            "case {index}"
        );
        assert!(!fixture.0.exists());
    }
}

#[tokio::test]
async fn all_protocols_loopback_urls_unicode_and_metadata_round_trip() {
    let fixture = Fixture::new();
    for (index, protocol) in [
        AiAgentProtocol::OpenaiResponses,
        AiAgentProtocol::OpenaiChatCompletions,
        AiAgentProtocol::AnthropicMessages,
        AiAgentProtocol::GeminiGenerateContent,
        AiAgentProtocol::Custom,
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = update(index as u32);
        request.profiles[0].protocol = protocol;
        request.profiles[0].name = " \u{feff} 本地模型 🐈 \u{feff} ".into();
        request.profiles[0].model = "\u{feff}example-model\u{feff}".into();
        request.profiles[0].base_url = [
            "https://example.test/v1",
            "http://localhost:1234/",
            "http://[::1]:1234/v1",
            "http://127.0.0.1:1234",
            "https://example.test/",
        ][index]
            .into();
        request.profiles[0].credential_ref = None;
        let expected = request.profiles.clone();
        assert_eq!(
            fixture.store().update(request).await.unwrap().profiles,
            expected
        );
    }
}

#[tokio::test]
async fn unicode_character_limits_are_independent_from_the_whole_body_byte_budget() {
    let fixture = Fixture::new();
    let mut request = update(0);
    request.profiles[0].name = "猫".repeat(128);
    request.profiles[0].model = "🐈".repeat(128);
    request.profiles[0].base_url = format!("https://example.test/{}", "猫".repeat(2027));
    assert_eq!(request.profiles[0].base_url.chars().count(), 2048);
    let expected = request.profiles.clone();
    assert_eq!(
        fixture.store().update(request).await.unwrap().profiles,
        expected
    );
    for field in ["name", "model", "base_url"] {
        let mut invalid = serde_json::to_value(update(1)).unwrap();
        invalid["profiles"][0][field] = if field == "base_url" {
            json!(format!("https://example.test/{}", "猫".repeat(2028)))
        } else {
            json!("🐈".repeat(129))
        };
        assert_eq!(
            fixture
                .store()
                .update(serde_json::from_value(invalid).unwrap())
                .await,
            Err(AiAgentSettingsError::InvalidInput)
        );
    }
    let mut excessive = update(1);
    excessive.default_profile_id = None;
    excessive.profiles = (0..16)
        .map(|index| {
            let mut item = update(0).profiles.remove(0);
            item.id = format!("agent_{index}");
            item.base_url = format!("https://example.test/{}", "猫".repeat(2027));
            item
        })
        .collect();
    assert_eq!(
        fixture.store().update(excessive).await,
        Err(AiAgentSettingsError::InvalidInput)
    );
}

#[test]
fn json_contract_rejects_secret_fields_unknown_protocol_duplicates_and_arrays() {
    let baseline = serde_json::to_value(update(0)).unwrap();
    for at_profile in [false, true] {
        for key in ["api_key", "headers", "token", "provider_secret"] {
            let mut value = baseline.clone();
            if at_profile {
                value["profiles"][0][key] = json!("not-a-real-secret");
            } else {
                value[key] = json!("not-a-real-secret");
            }
            assert!(serde_json::from_value::<UpdateAiAgentSettingsRequest>(value).is_err());
        }
    }
    for value in [json!("unsupported"), json!({"custom": null})] {
        let mut request = baseline.clone();
        request["profiles"][0]["protocol"] = value;
        assert!(serde_json::from_value::<UpdateAiAgentSettingsRequest>(request).is_err());
    }
    for source in [
        r#"{"expected_revision":0,"expected_revision":0,"profiles":[]}"#,
        r#"[0,null,[]]"#,
        r#"{"expected_revision":0,"profiles":[["a","A","custom","https://example.test","m",null,true]]}"#,
    ] {
        assert!(serde_json::from_str::<UpdateAiAgentSettingsRequest>(source).is_err());
    }
}

#[test]
fn nullable_fields_must_be_present_in_requests_profiles_and_saved_settings() {
    let mut request = serde_json::to_value(update(0)).unwrap();
    request
        .as_object_mut()
        .unwrap()
        .remove("default_profile_id");
    assert!(serde_json::from_value::<UpdateAiAgentSettingsRequest>(request).is_err());
    let mut request = serde_json::to_value(update(0)).unwrap();
    request["profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("credential_ref");
    assert!(serde_json::from_value::<UpdateAiAgentSettingsRequest>(request).is_err());
    assert!(
        serde_json::from_str::<cyanrex_engine::models::ai_agent::AiAgentSettings>(
            r#"{"revision":0,"profiles":[]}"#
        )
        .is_err()
    );
}

#[tokio::test]
async fn raw_url_whitespace_and_noncanonical_loopback_forms_are_rejected() {
    let fixture = Fixture::new();
    for base_url in [
        "https://example.test/a b",
        "https://example.test/a\u{2003}b",
        "http://127.1/v1",
        "http://2130706433/v1",
        "http://127.0.0.2/v1",
        "https://example.test/\u{feff}key",
        "https://*.example",
        "https://*",
    ] {
        let mut request = update(0);
        request.profiles[0].base_url = base_url.into();
        assert_eq!(
            fixture.store().update(request).await,
            Err(AiAgentSettingsError::InvalidInput)
        );
    }
    assert!(!fixture.0.exists());
}

#[tokio::test]
async fn raw_url_empty_ports_are_rejected_without_rejecting_valid_numeric_ports() {
    let fixture = Fixture::new();
    for base_url in [
        "http://localhost:/v1",
        "http://[::1]:/v1",
        "https://example.test:/v1",
        "https://[::1]:/v1",
        "http://127.0.0.1:+80/v1",
        "http://localhost:８０/v1",
        "https://example.test:65536/v1",
    ] {
        let mut request = update(0);
        request.profiles[0].base_url = base_url.into();
        assert_eq!(
            fixture.store().update(request).await,
            Err(AiAgentSettingsError::InvalidInput)
        );
    }
    assert!(!fixture.0.exists());
    for (index, base_url) in [
        "http://localhost/v1",
        "http://localhost:0/v1",
        "http://127.0.0.1:00080/v1",
        "http://[::1]:65535/v1",
        "https://example.test:443/v1",
        "https://[::1]:443/v1",
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = update(index as u32);
        request.profiles[0].base_url = base_url.into();
        assert_eq!(
            fixture.store().update(request).await.unwrap().profiles[0].base_url,
            base_url
        );
    }
}
