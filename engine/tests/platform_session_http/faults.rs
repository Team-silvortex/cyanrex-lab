use super::fixture::*;
use axum::http::StatusCode;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_http_suppressed_or_redirected_logout_never_clears_a_still_valid_cookie() {
    for redirect in [false, true] {
        let (reply, before, after, hit, valid, restored) = Fixture::checked(|f| {
            Box::pin(async move {
                if redirect {
                    let shadow = f.shadow().await;
                    f.fault(
                        "AFTER DELETE",
                        &format!(
                            "PERFORM set_config('search_path', '{shadow}', true); RETURN OLD;"
                        ),
                    )
                    .await;
                } else {
                    f.fault("BEFORE DELETE", "RETURN NULL;").await;
                }
                let before = f.snapshot().await;
                let reply = capture(f.app(), request("POST", LOGOUT, &f.token)).await;
                let hit = f.calls().await;
                let restored = f.drain().await;
                let after = f.snapshot().await;
                let valid = within(f.source().validate_session(&f.token))
                    .await
                    .unwrap_or_else(|_| panic!("retained Session control failed"))
                    .is_some();
                (reply, before, after, hit, valid, restored)
            })
        })
        .await;
        denied(
            &reply,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
        assert_eq!(
            hit,
            (true, 1),
            "logout did not reach its actual DELETE trigger"
        );
        assert!(
            unchanged(&before, &after),
            "failed logout changed original rows or relation pins"
        );
        assert!(
            restored && valid,
            "failure left an adopted path or revoked the original Session"
        );
        assert_eq!(after.session_count, 2);
    }
}
