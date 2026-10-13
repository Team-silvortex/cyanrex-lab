use crate::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_requires_exact_current_recipient_and_rechecks_revocation_after_body_wait(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let route = bounded(f);
            let uri = path(&f.request());
            for token in [&auth(f).learner_token, &auth(f).manager_token] {
                denied(
                    f,
                    &route,
                    request(token, &uri),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
            }
            for (field, value) in [
                ("file_id", serde_json::json!(Uuid::new_v4())),
                ("version_id", serde_json::json!(Uuid::new_v4())),
                ("byte_length", serde_json::json!(18)),
                ("sha256", serde_json::json!("b".repeat(64))),
            ] {
                let mut typed = f.request();
                let mut raw = serde_json::to_value(&typed.expected_file).unwrap();
                raw[field] = value;
                typed.expected_file = serde_json::from_value(raw).unwrap();
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &path(&typed)),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
            }
            let mut missing = f.request();
            missing.grant_id = sharing_fixture::recipient_grant(99);
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&missing)),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
            missing = f.request();
            missing.upload_id = publication_fixture::upload(99);
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&missing)),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
            for (seed, window) in [
                (2, FileGrantValidity::new(1, 2).unwrap()),
                (
                    3,
                    FileGrantValidity::new(
                        MAX_FILE_GRANT_UNIX_SECONDS - 1,
                        MAX_FILE_GRANT_UNIX_SECONDS,
                    )
                    .unwrap(),
                ),
            ] {
                download_support::own_grant(f, seed, window).await;
                let typed = download_support::request(f, seed);
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &path(&typed)),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
            }
            healthy(f, &route).await;
            let held = Held::start(route.clone(), &auth(f).target_token, &uri).await;
            saturated(&route, &uri).await;
            // Actual body polling has started, but Source selection cannot start before empty EOF.
            f.sharing
                .store
                .revoke(
                    f.request().grant_id,
                    f.sharing.actor(),
                    sharing_fixture::revision(1),
                )
                .await
                .unwrap();
            let revoked = f.sharing.snapshot().await;
            error_response(
                held.finish().await,
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            assert_eq!(f.sharing.snapshot().await, revoked);
            denied(
                f,
                &route,
                request(&auth(f).target_token, &uri),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
            download_support::drain(f).await;
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_manifest_http_maps_current_session_membership_and_workspace_lifecycle_privately()
{
    checked(Fixture::ready().await, |f| Box::pin(async move {
        let route = app(f);
        let uri = path(&f.request());
        healthy(f, &route).await;
        download_support::membership(f, f.sharing.recipient.principal().principal_id, MembershipStatus::Suspended).await;
        denied(f, &route, request(&auth(f).target_token, &uri), StatusCode::FORBIDDEN, "access_denied").await;
        download_support::membership(f, f.sharing.recipient.principal().principal_id, MembershipStatus::Active).await;
        healthy(f, &route).await;
        download_support::sql(f, &format!("UPDATE {}.collaboration_workspaces SET status='archived'", auth(f).base.schema)).await;
        denied(f, &route, request(&auth(f).target_token, &uri), StatusCode::FORBIDDEN, "access_denied").await;
        download_support::sql(f, &format!("UPDATE {}.collaboration_workspaces SET status='active'", auth(f).base.schema)).await;
        healthy(f, &route).await;
        query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1")
            .bind(source_fixture::token_hash(&auth(f).target_token)).execute(&auth(f).base.pool).await.unwrap();
        denied(f, &route, request(&auth(f).target_token, &uri), StatusCode::UNAUTHORIZED, "invalid_session").await;
        download_support::refresh_expiry(f).await;
        healthy(f, &route).await;
        auth(f).source.logout(&auth(f).target_token).await.unwrap();
        denied(f, &route, request(&auth(f).target_token, &uri), StatusCode::UNAUTHORIZED, "invalid_session").await;
        let login = auth(f).base.seed_session(&auth(f).target).await;
        f.sharing.base.grants.auth.target_token = login.token;
        healthy(f, &route).await;
        download_support::drain(f).await;
    })).await;
}
