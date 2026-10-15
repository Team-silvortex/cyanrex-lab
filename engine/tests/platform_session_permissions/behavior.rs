use super::{fixture::Restricted, http_fixture::*};
use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_permissions_column_only_role_reads_and_logs_out_without_credentials() {
    let (
        before,
        read_only,
        after,
        first,
        logout,
        rejected,
        other,
        exact_role,
        acl,
        denied_credentials,
        absent,
        retained,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            Restricted::checked(f, |r| {
                Box::pin(async move {
                    let f = r.owner;
                    let exact_role = r.exact_role().await;
                    let acl = r.minimal_acl().await;
                    let denied_credentials = r.credential_projection_denied().await;
                    let before = f.snapshot().await;
                    let first = capture(r.app(), request("GET", SESSION, &f.token)).await;
                    let read_only = f.snapshot().await;
                    let logout = capture(r.app(), request("POST", LOGOUT, &f.token)).await;
                    let rejected = capture(r.app(), request("GET", SESSION, &f.token)).await;
                    let other = capture(r.app(), request("GET", SESSION, &f.other_token)).await;
                    let after = f.snapshot().await;
                    let absent = within(f.source().validate_session(&f.token))
                        .await
                        .unwrap_or_else(|_| panic!("privileged revoked Session control failed"))
                        .is_none();
                    let retained = within(f.source().validate_session(&f.other_token))
                        .await
                        .unwrap_or_else(|_| panic!("privileged other-device control failed"))
                        .is_some();
                    (
                        before,
                        read_only,
                        after,
                        first,
                        logout,
                        rejected,
                        other,
                        exact_role,
                        acl,
                        denied_credentials,
                        absent,
                        retained,
                    )
                })
            })
            .await
        })
    })
    .await;

    // All expectations follow restricted-pool/role and original namespace cleanup, including Red.
    assert!(
        exact_role && acl && denied_credentials,
        "fixture was not a genuine credential-blind login role"
    );
    successful(
        &first,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    successful(&logout, json!({"schema_version":1,"ok":true}), true);
    denied(
        &rejected,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "unconfirmed",
    );
    successful(
        &other,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    assert!(
        unchanged(&before, &read_only),
        "restricted GET changed Source state"
    );
    assert!(
        before.users == after.users && before.source == after.source,
        "logout touched credentials, OTP state or Source authority"
    );
    assert_eq!(before.session_count, 2);
    assert_eq!(after.session_count, 1);
    assert!(
        before.registry_absent && after.registry_absent && absent && retained,
        "restricted lifecycle acquired a registry or revoked the other device"
    );
    assert!(
        before.namespace == after.namespace && before.relations == after.relations,
        "restricted lifecycle replaced original storage"
    );
}
