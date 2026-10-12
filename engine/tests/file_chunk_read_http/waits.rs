use crate::{fixture::*, source_gate, support::*, *};
use std::time::Duration;
async fn expire_soon(f: &Fixture) {
    query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1")
        .bind(source_fixture::token_hash(&auth(f).target_token))
        .execute(&auth(f).base.pool)
        .await
        .unwrap();
}
async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let yes: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                auth(f).base.schema
            ))
            .bind(source_fixture::token_hash(&auth(f).target_token))
            .fetch_one(&auth(f).base.admin)
            .await
            .unwrap()
            .get("yes");
            if yes {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_http_real_final_wait_expiry_cancel_and_lease_cleanup_discard_private_bytes()
{
    for mode in ["session", "grant", "cancel"] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            let cancel=mode=="cancel";
            let mut typed=f.request();
            let boundary=if mode=="grant" {
                let now:i64=query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS n").fetch_one(&auth(f).base.admin).await.unwrap().get("n");let boundary=now+2;
                f.sharing.store.issue(sharing_fixture::recipient_grant(2),f.sharing.actor(),f.sharing.expected_file.clone(),f.sharing.actor(),f.sharing.recipient,FileGrantValidity::new(0,boundary as u64).unwrap()).await.unwrap();typed.download.grant_id=sharing_fixture::recipient_grant(2);Some(boundary)
            }else {if mode=="session" {expire_soon(f).await;}None};
            let before=f.sharing.snapshot().await;let (source,w,release,started,_)=source_gate::captured(f).await;
            let route=build_file_chunk_read_router(FileChunkReadHttpState::with_capacity(source.clone(),w.clone(),ORIGIN,1).unwrap());let uri=path(&typed);
            let mut pending=Box::pin(route.clone().oneshot(request(&auth(f).target_token,&uri)));let mut early=None;
            let committed=tokio::select! {r=&mut pending=>{early=Some(r.unwrap());false},r=tokio::time::timeout(Duration::from_secs(3),started)=>r.is_ok_and(|r|r.is_ok())};
            let mut held=holder(f).await;let blocker=backend(&mut held).await;drop(release);
            let waited=if early.is_some(){false}else{tokio::select! {r=&mut pending=>{early=Some(r.unwrap());false},yes=blocked(f,blocker)=>yes}};
            // The generic release hook alone proves transaction end/connection return, not
            // COMMIT versus rollback. Together with this later real marker wait and the Source's
            // two-phase control flow, it identifies the successful initial ACK and FINAL
            // transaction after private checked bytes. This is not a file-worker callback.
            saturated(&route,&uri).await;
            assert_eq!(source.read_session_file_chunk(&auth(f).target_token,&w,&typed).await.err(),Some(cyanrex_engine::services::auth_service::durable_source::SessionFileChunkReadError::CapacityExhausted),"the SAME captured workspace must retain its private byte lease");
            if cancel {
                drop(pending);held.rollback().await.unwrap();drain(f).await;assert!(early.is_none());drop(early);
            }else {
                let elapsed=if waited {if let Some(boundary)=boundary {
                    tokio::time::timeout(Duration::from_secs(2),async {loop {let yes:bool=query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint >= $1 AS yes").bind(boundary).fetch_one(&auth(f).base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
                }else{expired(f).await}}else{false};held.rollback().await.unwrap();
                let response=match early {Some(r)=>r,None=>pending.as_mut().await.unwrap()};drop(pending);
                if mode=="session" {error_response(response,StatusCode::UNAUTHORIZED,"invalid_session","unconfirmed").await;}
                else {error_response(response,StatusCode::NOT_FOUND,"not_found","unconfirmed").await;}
                assert!(elapsed,"expiry must be demonstrated with PostgreSQL's current clock while truly blocked");
            }
            assert_eq!(f.sharing.snapshot().await,before);
            if mode=="session" {query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token=$1").bind(source_fixture::token_hash(&auth(f).target_token)).execute(&auth(f).base.pool).await.unwrap();}
            healthy(f,&route).await;assert!(committed&&waited,"both actual first ACK and final SQL blocker are required");drain(f).await;
        })).await;
    }
    // The cancelled future is dropped during a known pre-COMMIT SELECT wait. These read-only
    // snapshots do not prove arbitrary lost acknowledgement, network delivery or LAN/TLS behavior.
}
