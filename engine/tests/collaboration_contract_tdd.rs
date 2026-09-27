use cyanrex_engine::models::collaboration::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const ID: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";
const OTHER_ID: &str = "267c6b0f-182c-4980-a925-cdc967327c57";

fn round_trip<T: Serialize + DeserializeOwned>(value: Value) {
    let parsed: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
}

fn workspace() -> Value {
    json!({"authority_id": ID, "workspace_id": OTHER_ID})
}

fn artifact() -> Value {
    json!({
        "workspace": workspace(),
        "artifact_id": ID,
        "revision_id": OTHER_ID,
        "sha256": "a".repeat(64)
    })
}

fn envelope() -> Value {
    json!({
        "schema_version": 1,
        "event_id": ID,
        "workspace": workspace(),
        "aggregate": {"kind": "artifact", "id": ID},
        "aggregate_revision": 1,
        "actor": {"authority_id": ID, "principal_id": OTHER_ID},
        "event_type": "cyanrex.artifact.revision_published",
        "occurred_at": "2026-09-27T00:00:00Z",
        "recorded_at": "2026-09-27T00:00:01Z",
        "correlation_id": OTHER_ID,
        "causation_id": null,
        "payload_ref": artifact()
    })
}

#[test]
fn core_ids_are_non_nil_canonical_uuid_strings_without_implicit_allocation() {
    macro_rules! check_ids {
        ($($kind:ty),+ $(,)?) => {$(
            round_trip::<$kind>(json!(ID));
            let parsed: $kind = ID.parse().unwrap();
            assert_eq!(parsed.to_string(), ID);
            assert_eq!(parsed.as_uuid(), uuid::Uuid::parse_str(ID).unwrap());
            assert_eq!(<$kind>::try_from(parsed.as_uuid()).unwrap(), parsed);
            assert!(<$kind>::try_from(uuid::Uuid::nil()).is_err());
            for invalid in [
                "", "admin", "00000000-0000-0000-0000-000000000000",
                "8b075b0b81184c379224337ba22f40c6",
                "8B075B0B-8118-4C37-9224-337BA22F40C6",
            ] {
                assert!(serde_json::from_value::<$kind>(json!(invalid)).is_err());
                assert!(invalid.parse::<$kind>().is_err());
            }
            assert!(serde_json::from_value::<$kind>(Value::Null).is_err());
            assert!(serde_json::from_value::<$kind>(json!(42)).is_err());
        )+};
    }
    check_ids!(
        AuthorityId,
        PrincipalId,
        WorkspaceId,
        ArtifactId,
        ArtifactRevisionId,
        TaskId,
        RunId,
        ReviewId,
        EventId,
        CorrelationId,
        LegacyAccountId,
    );
}

#[test]
fn identity_and_membership_do_not_encode_global_teacher_authority() {
    for kind in ["human", "agent", "service", "machine"] {
        round_trip::<Principal>(json!({
            "reference": {"authority_id": ID, "principal_id": OTHER_ID},
            "kind": kind, "display_name": "Synthetic participant", "status": "active"
        }));
    }
    round_trip::<Workspace>(json!({
        "reference": workspace(), "title": "Synthetic workspace", "status": "active"
    }));
    let membership = json!({
        "workspace": workspace(), "principal_id": ID,
        "role_refs": ["cyanrex.workspace.owner"], "status": "active"
    });
    round_trip::<Membership>(membership.clone());
    let mut forged = membership;
    forged["deployment_admin"] = json!(true);
    assert!(serde_json::from_value::<Membership>(forged).is_err());
    assert!(serde_json::from_value::<PrincipalKind>(json!("teacher")).is_err());
    assert!(serde_json::from_value::<PrincipalKind>(json!("runner_agent")).is_err());
}

#[test]
fn refs_always_include_authority_and_workspace_not_just_local_ids() {
    let original: ArtifactRef = serde_json::from_value(artifact()).unwrap();
    let mut another_instance = artifact();
    another_instance["workspace"]["authority_id"] = json!(OTHER_ID);
    let other: ArtifactRef = serde_json::from_value(another_instance).unwrap();
    assert_ne!(original, other);
    let mut another_workspace = artifact();
    another_workspace["workspace"]["workspace_id"] = json!(ID);
    let other: ArtifactRef = serde_json::from_value(another_workspace).unwrap();
    assert_ne!(original, other);
    for field in ["workspace", "artifact_id", "revision_id", "sha256"] {
        let mut incomplete = artifact();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ArtifactRef>(incomplete).is_err());
    }
    let mut missing_authority = artifact();
    missing_authority["workspace"]
        .as_object_mut()
        .unwrap()
        .remove("authority_id");
    assert!(serde_json::from_value::<ArtifactRef>(missing_authority).is_err());
    round_trip::<ArtifactRef>(artifact());
}

#[test]
fn revision_and_content_digest_are_part_of_artifact_identity() {
    let original: ArtifactRef = serde_json::from_value(artifact()).unwrap();
    for (field, replacement) in [
        ("revision_id", json!(ID)),
        ("sha256", json!("b".repeat(64))),
    ] {
        let mut changed = artifact();
        changed[field] = replacement;
        let changed: ArtifactRef = serde_json::from_value(changed).unwrap();
        assert_ne!(original, changed);
    }
    for invalid in [
        "".to_owned(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
    ] {
        assert!(invalid.parse::<Sha256Digest>().is_err());
        let mut changed = artifact();
        changed["sha256"] = json!(invalid);
        assert!(serde_json::from_value::<ArtifactRef>(changed).is_err());
    }
}

#[test]
fn contract_version_and_revision_reject_unsupported_or_lossy_numbers() {
    round_trip::<EventEnvelope>(envelope());
    for invalid in [json!(0), json!(2), json!("1"), json!(1.5), Value::Null] {
        let mut event = envelope();
        event["schema_version"] = invalid;
        assert!(serde_json::from_value::<EventEnvelope>(event).is_err());
    }
    for invalid in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!("1"),
        json!(9_007_199_254_740_992_u64),
    ] {
        assert!(serde_json::from_value::<RevisionNumber>(invalid).is_err());
    }
    round_trip::<RevisionNumber>(json!(9_007_199_254_740_991_u64));
}

#[test]
fn names_are_bounded_namespaced_identifiers_not_free_form_commands() {
    for valid in ["cyanrex.teaching.teacher", "cyanrex.run.finished_v1"] {
        round_trip::<QualifiedName>(json!(valid));
    }
    for invalid in [
        "", "teacher", ".teacher", "teacher.", "a..b", "A.b", "a.1", "a.*", "a.b c",
    ] {
        assert!(invalid.parse::<QualifiedName>().is_err());
        assert!(serde_json::from_value::<QualifiedName>(json!(invalid)).is_err());
    }
    assert!(format!("a.{}", "b".repeat(127))
        .parse::<QualifiedName>()
        .is_err());
}

#[test]
fn event_contract_preserves_unknown_facts_without_inventing_history() {
    let mut imported = envelope();
    imported["occurred_at"] = Value::Null;
    imported["actor"] = Value::Null;
    imported["payload_ref"] = Value::Null;
    round_trip::<EventEnvelope>(imported);
    let mut future = envelope();
    future["trusted_audit"] = json!(true);
    assert!(serde_json::from_value::<EventEnvelope>(future).is_err());
    let mut missing_time = envelope();
    missing_time.as_object_mut().unwrap().remove("recorded_at");
    assert!(serde_json::from_value::<EventEnvelope>(missing_time).is_err());
    let mut malformed = envelope();
    malformed["recorded_at"] = json!("yesterday");
    assert!(serde_json::from_value::<EventEnvelope>(malformed).is_err());
}

#[test]
fn old_telemetry_cannot_be_deserialized_as_a_business_event() {
    let telemetry = json!({
        "username": "synthetic-student", "timestamp": "2026-09-27T00:00:00Z",
        "source": "kernel", "event_type": "ebpf.debug_breakpoint_hit", "payload": {}
    });
    assert!(serde_json::from_value::<EventEnvelope>(telemetry).is_err());
}

#[test]
fn legacy_identity_binding_keeps_canonical_username_separate_from_principal_id() {
    let binding = json!({"authority_id": ID, "username": "student-01", "principal_id": OTHER_ID});
    round_trip::<LegacyIdentityBinding>(binding.clone());
    for invalid in [
        "",
        "ab",
        "Student-01",
        " student-01 ",
        "../student",
        "student@example.com",
    ] {
        let mut changed = binding.clone();
        changed["username"] = json!(invalid);
        assert!(serde_json::from_value::<LegacyIdentityBinding>(changed).is_err());
    }
    assert!("a".repeat(65).parse::<LegacyUsername>().is_err());
    assert!("a".repeat(64).parse::<LegacyUsername>().is_ok());
    let mut missing_id = binding;
    missing_id.as_object_mut().unwrap().remove("principal_id");
    assert!(serde_json::from_value::<LegacyIdentityBinding>(missing_id).is_err());
}
