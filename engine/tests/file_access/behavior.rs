use super::{fixture::*, *};

#[test]
fn active_publisher_matches_only_exact_account_inside_half_open_window() {
    let candidate = publisher(FileGrantState::Active);
    assert!(
        candidate.matches(scope(), account(1), 100),
        "active exact publisher must match at inclusive lower bound"
    );
    assert!(candidate.matches(scope(), account(1), 199));
    assert!(!candidate.matches(scope(), account(1), 99));
    assert!(!candidate.matches(scope(), account(1), 200));
    assert!(!candidate.matches(scope(), account(1), u64::MAX));
    assert_eq!(candidate.id(), grant(1));
    assert_eq!(candidate.revision(), revision(3));
    assert_eq!(candidate.workspace(), scope());
    assert_eq!(candidate.subject(), account(1));
    assert_eq!(candidate.validity().not_before(), 100);
    assert_eq!(candidate.validity().expires_at(), 200);
    assert_eq!(candidate.state(), FileGrantState::Active);
    assert_eq!(u8::from(candidate.schema_version()), 1);
    let mut foreign = scope();
    foreign.workspace_id = Uuid::from_u128(0xb2).try_into().unwrap();
    assert!(!candidate.matches(foreign, account(1), 150));
    foreign = scope();
    foreign.authority_id = Uuid::from_u128(0xa2).try_into().unwrap();
    assert!(!candidate.matches(foreign, account(1), 150));
    for subject in [
        account(2),
        FileAccountRef::new(account(2).principal(), account(1).account_id()),
        FileAccountRef::new(account(1).principal(), account(2).account_id()),
        FileAccountRef::new(
            PrincipalRef {
                authority_id: foreign.authority_id,
                principal_id: account(1).principal().principal_id,
            },
            account(1).account_id(),
        ),
    ] {
        assert!(!candidate.matches(scope(), subject, 150));
    }
}
