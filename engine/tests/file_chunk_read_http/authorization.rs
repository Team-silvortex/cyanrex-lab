use crate::{fixture::*, source_gate, support::*, *};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_current_recipient_lifecycle_and_original_gap_failures_never_emit_bytes(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            let route = app(f);
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
            let mut missing = f.request();
            missing.download.grant_id = sharing_fixture::recipient_grant(99);
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&missing)),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
            for (field, value) in [
                ("file_id", serde_json::json!(Uuid::new_v4())),
                ("version_id", serde_json::json!(Uuid::new_v4())),
                ("byte_length", serde_json::json!(18)),
                ("sha256", serde_json::json!("b".repeat(64))),
            ] {
                let mut r = f.request();
                let mut raw = serde_json::to_value(&r.download.expected_file).unwrap();
                raw[field] = value;
                r.download.expected_file = serde_json::from_value(raw).unwrap();
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &path(&r)),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
            }
            let mut missing = f.request();
            missing.download.upload_id = publication_fixture::upload(99);
            denied(
                f,
                &route,
                request(&auth(f).target_token, &path(&missing)),
                StatusCode::NOT_FOUND,
                "not_found",
            )
            .await;
            for (seed, v) in [
                (2, FileGrantValidity::new(1, 2).unwrap()),
                (
                    3,
                    FileGrantValidity::new(
                        MAX_FILE_GRANT_UNIX_SECONDS - 1,
                        MAX_FILE_GRANT_UNIX_SECONDS,
                    )
                    .unwrap(),
                ),
                (4, grant_fixture::validity()),
            ] {
                f.sharing
                    .store
                    .issue(
                        sharing_fixture::recipient_grant(seed),
                        f.sharing.actor(),
                        f.sharing.expected_file.clone(),
                        f.sharing.actor(),
                        f.sharing.recipient,
                        v,
                    )
                    .await
                    .unwrap();
                if seed == 4 {
                    f.sharing
                        .store
                        .revoke(
                            sharing_fixture::recipient_grant(seed),
                            f.sharing.actor(),
                            sharing_fixture::revision(1),
                        )
                        .await
                        .unwrap();
                }
                let mut r = f.request();
                r.download.grant_id = sharing_fixture::recipient_grant(seed);
                denied(
                    f,
                    &route,
                    request(&auth(f).target_token, &path(&r)),
                    StatusCode::NOT_FOUND,
                    "not_found",
                )
                .await;
            }
            let current = auth(f)
                .current(f.sharing.recipient.principal().principal_id)
                .await;
            let mut desired = current.policy.clone();
            desired.membership.status = MembershipStatus::Suspended;
            auth(f)
                .source
                .apply_session_policy_command(
                    &auth(f).manager_token,
                    &SessionPolicyCommand {
                        command_id: id(),
                        expected_revision: Some(current.revision),
                        desired,
                    },
                )
                .await
                .unwrap();
            denied(
                f,
                &route,
                request(&auth(f).target_token, &uri),
                StatusCode::FORBIDDEN,
                "access_denied",
            )
            .await;
            let current = auth(f)
                .current(f.sharing.recipient.principal().principal_id)
                .await;
            let mut desired = current.policy;
            desired.membership.status = MembershipStatus::Active;
            auth(f)
                .source
                .apply_session_policy_command(
                    &auth(f).manager_token,
                    &SessionPolicyCommand {
                        command_id: id(),
                        expected_revision: Some(current.revision),
                        desired,
                    },
                )
                .await
                .unwrap();
            healthy(f, &route).await;
            auth(f).source.logout(&auth(f).target_token).await.unwrap();
            denied(
                f,
                &route,
                request(&auth(f).target_token, &uri),
                StatusCode::UNAUTHORIZED,
                "invalid_session",
            )
            .await;
            let login = auth(f).base.seed_session(&auth(f).target).await;
            f.sharing.base.grants.auth.target_token = login.token;
            healthy(f, &route).await;
            drain(f).await;
        })
    })
    .await;
    for revoked in [true, false] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            let (source,w,release,started,_)=source_gate::captured(f).await;
            let route=build_file_chunk_read_router(FileChunkReadHttpState::with_capacity(source,w,ORIGIN,1).unwrap());let uri=path(&f.request());
            let mut pending=Box::pin(route.clone().oneshot(request(&auth(f).target_token,&uri)));let mut early=None;
            let committed=tokio::select! {r=&mut pending=>{early=Some(r.unwrap());false},r=tokio::time::timeout(Duration::from_secs(3),started)=>r.is_ok_and(|r|r.is_ok())};
            let mut writer=auth(f).base.admin.begin().await.unwrap();
            let old_nonce:Uuid=query(&format!("SELECT nonce FROM {}.{PUB_ROWS} WHERE upload_id=$1",f.sharing.base.publication_schema)).bind(f.request().download.upload_id.as_uuid()).fetch_one(&mut *writer).await.unwrap().get("nonce");
            if revoked {writer.rollback().await.unwrap();f.sharing.store.revoke(f.request().download.grant_id,f.sharing.actor(),sharing_fixture::revision(1)).await.unwrap();writer=auth(f).base.admin.begin().await.unwrap();}
            else {query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(Uuid::new_v4()).bind(f.request().download.upload_id.as_uuid()).execute(&mut *writer).await.unwrap();}
            let injected=snapshot_in_tx(f,&mut writer).await;writer.commit().await.unwrap();drop(release);
            let response=match early {Some(r)=>r,None=>pending.as_mut().await.unwrap()};drop(pending);error_response(response,StatusCode::SERVICE_UNAVAILABLE,"service_unavailable","unconfirmed").await;
            assert_eq!(f.sharing.snapshot().await,injected);assert!(committed,"original selected pins must cross a real first COMMIT, not an elapsed sleep");
            if revoked {f.sharing.store.issue(sharing_fixture::recipient_grant(2),f.sharing.actor(),f.sharing.expected_file.clone(),f.sharing.actor(),f.sharing.recipient,grant_fixture::validity()).await.unwrap();
                let mut r=f.request();r.download.grant_id=sharing_fixture::recipient_grant(2);assert_eq!(binary(f,&route,&r).await,(0..17).map(bytes::byte).collect::<Vec<_>>());}
            else {query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(old_nonce).bind(f.request().download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();healthy(f,&route).await;}
            drain(f).await;
        })).await;
    }
}
