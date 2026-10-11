use super::{fixture::*, *};

#[test]
fn publisher_and_recipient_roundtrip_exact_schema_one_objects() {
    let publisher = publisher(FileGrantState::Active);
    let raw = raw_publisher();
    assert_eq!(serde_json::to_value(&publisher).unwrap(), raw);
    let decoded = FilePublisherGrant::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), raw);
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<FilePublisherGrant>(raw.clone()).unwrap())
            .unwrap(),
        raw
    );
    let recipient = recipient(FileGrantState::Active);
    let raw = raw_recipient();
    assert_eq!(serde_json::to_value(&recipient).unwrap(), raw);
    assert_eq!(recipient.id(), grant(2));
    assert_eq!(recipient.revision(), revision(5));
    assert_eq!(recipient.file(), &file());
    assert_eq!(recipient.publisher(), account(1));
    assert_eq!(recipient.recipient(), account(2));
    assert_eq!(recipient.validity().not_before(), 100);
    assert_eq!(recipient.validity().expires_at(), 200);
    assert_eq!(recipient.state(), FileGrantState::Active);
    assert_eq!(u8::from(recipient.schema_version()), 1);
    let decoded = FileRecipientGrant::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), raw);
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<FileRecipientGrant>(raw.clone()).unwrap())
            .unwrap(),
        raw
    );
    for (state, text) in [
        (FileGrantState::Active, "active"),
        (FileGrantState::Revoked, "revoked"),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(text));
        assert_eq!(
            serde_json::from_value::<FileGrantState>(json!(text)).unwrap(),
            state
        );
    }
    for bad in ["Active", "Revoked", "published", "unknown", "", "allow"] {
        assert!(serde_json::from_value::<FileGrantState>(json!(bad)).is_err());
    }
}

#[test]
fn constructors_and_decode_reject_cross_authority_attribution() {
    let foreign = FileAccountRef::new(
        PrincipalRef {
            authority_id: Uuid::from_u128(0xa2).try_into().unwrap(),
            principal_id: account(1).principal().principal_id,
        },
        account(1).account_id(),
    );
    assert!(FilePublisherGrant::new(
        grant(1),
        revision(1),
        scope(),
        foreign,
        validity(),
        FileGrantState::Active
    )
    .is_err());
    assert!(FileRecipientGrant::new(
        grant(2),
        revision(1),
        file(),
        foreign,
        account(2),
        validity(),
        FileGrantState::Active
    )
    .is_err());
    let foreign = FileAccountRef::new(
        PrincipalRef {
            authority_id: Uuid::from_u128(0xa2).try_into().unwrap(),
            principal_id: account(2).principal().principal_id,
        },
        account(2).account_id(),
    );
    assert!(FileRecipientGrant::new(
        grant(2),
        revision(1),
        file(),
        account(1),
        foreign,
        validity(),
        FileGrantState::Active
    )
    .is_err());
    let mut pub_raw = raw_publisher();
    pub_raw["subject"]["principal"]["authority_id"] = json!(foreign.principal().authority_id);
    reject_publisher(&pub_raw);
    for field in ["publisher", "recipient"] {
        let mut raw = raw_recipient();
        raw[field]["principal"]["authority_id"] = json!(foreign.principal().authority_id);
        reject_recipient(&raw);
    }
    let mut raw = raw_recipient();
    raw["file"]["workspace"]["authority_id"] = json!(foreign.principal().authority_id);
    reject_recipient(&raw);
}

#[test]
fn active_recipient_matching_checks_every_file_and_account_pin() {
    let candidate = recipient(FileGrantState::Active);
    let expected = file();
    assert!(candidate.matches(&expected, account(1), account(2), 100));
    assert!(candidate.matches(&expected, account(1), account(2), 199));
    for now in [0, 99, 200, 201, u64::MAX] {
        assert!(!candidate.matches(&expected, account(1), account(2), now));
    }
    let raw = serde_json::to_value(&expected).unwrap();
    for (pointer, value) in [
        ("/workspace/authority_id", json!(Uuid::from_u128(0xa2))),
        ("/workspace/workspace_id", json!(Uuid::from_u128(0xb2))),
        ("/file_id", json!(Uuid::from_u128(0xf3))),
        ("/version_id", json!(Uuid::from_u128(0xf4))),
        ("/byte_length", json!(12)),
        ("/sha256", json!("b".repeat(64))),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let observed: FileVersionRef = serde_json::from_value(changed).unwrap();
        assert!(
            !candidate.matches(&observed, account(1), account(2), 150),
            "must reject exact pin drift at {pointer}"
        );
    }
    let foreign: AuthorityId = Uuid::from_u128(0xa2).try_into().unwrap();
    for publisher in [
        account(3),
        FileAccountRef::new(account(3).principal(), account(1).account_id()),
        FileAccountRef::new(account(1).principal(), account(3).account_id()),
        FileAccountRef::new(
            PrincipalRef {
                authority_id: foreign,
                principal_id: account(1).principal().principal_id,
            },
            account(1).account_id(),
        ),
    ] {
        assert!(!candidate.matches(&expected, publisher, account(2), 150));
    }
    for recipient in [
        account(3),
        FileAccountRef::new(account(3).principal(), account(2).account_id()),
        FileAccountRef::new(account(2).principal(), account(3).account_id()),
        FileAccountRef::new(
            PrincipalRef {
                authority_id: foreign,
                principal_id: account(2).principal().principal_id,
            },
            account(2).account_id(),
        ),
    ] {
        assert!(!candidate.matches(&expected, account(1), recipient, 150));
    }
    assert!(
        !candidate.matches(&expected, account(2), account(1), 150),
        "publisher and recipient are ordered attribution"
    );
}

#[test]
fn revocation_is_explicit_per_record_and_self_recipient_is_not_implicit_authority() {
    let revoked = publisher(FileGrantState::Revoked);
    assert!(!revoked.matches(scope(), account(1), 150));
    let independently_active = recipient(FileGrantState::Active);
    assert!(
        independently_active.matches(&file(), account(1), account(2), 150),
        "recipient record is an independent candidate, not tied to a publisher-grant ID/revision"
    );
    let revoked_recipient = recipient(FileGrantState::Revoked);
    assert!(!revoked_recipient.matches(&file(), account(1), account(2), 150));
    let self_recipient = FileRecipientGrant::new(
        grant(3),
        revision(1),
        file(),
        account(1),
        account(1),
        validity(),
        FileGrantState::Active,
    )
    .unwrap();
    assert!(self_recipient.matches(&file(), account(1), account(1), 150));
    assert!(
        !independently_active.matches(&file(), account(1), account(1), 150),
        "self access still needs its own explicitly supplied recipient candidate"
    );
    for state in [FileGrantState::Active, FileGrantState::Revoked] {
        let raw = serde_json::to_vec(&publisher(state)).unwrap();
        let p = FilePublisherGrant::parse_json(&raw).unwrap();
        assert_eq!(p.state(), state);
        let raw = serde_json::to_vec(&recipient(state)).unwrap();
        let r = FileRecipientGrant::parse_json(&raw).unwrap();
        assert_eq!(r.state(), state);
    }
}

#[test]
fn stale_active_copies_and_caller_claims_are_only_matching_candidates_not_current_authority() {
    let active = publisher(FileGrantState::Active);
    let exported = serde_json::to_vec(&active).unwrap();
    let revoked = FilePublisherGrant::new(
        active.id(),
        revision(4),
        scope(),
        account(1),
        validity(),
        FileGrantState::Revoked,
    )
    .unwrap();
    assert!(!revoked.matches(scope(), account(1), 150));
    let stale = FilePublisherGrant::parse_json(&exported).unwrap();
    assert_eq!(stale.revision(), revision(3));
    assert!(stale.matches(scope(),account(1),150),
        "pure parsing/matching cannot consult a later audit or revocation; no current permit is produced");
    let active = recipient(FileGrantState::Active);
    let exported = serde_json::to_vec(&active).unwrap();
    let revoked = FileRecipientGrant::new(
        active.id(),
        revision(6),
        file(),
        account(1),
        account(2),
        validity(),
        FileGrantState::Revoked,
    )
    .unwrap();
    assert!(!revoked.matches(&file(), account(1), account(2), 150));
    let stale = FileRecipientGrant::parse_json(&exported).unwrap();
    assert_eq!(stale.revision(), revision(5));
    assert!(stale.matches(&file(), account(1), account(2), 150));
}
