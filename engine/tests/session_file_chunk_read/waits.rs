use crate::{fixture::Fixture, pool_gate, support::*, *};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_final_wait_rechecks_exact_session_and_recipient_expiry_before_bytes_escape(
) {
    for session_expiry in [true, false] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            let boundary=if session_expiry {expire_soon(f).await;None}else{
                let boundary=now(f).await+2;own_grant(f,2,FileGrantValidity::new(0,boundary).unwrap()).await;Some(boundary)
            };
            let r=if session_expiry {f.request()}else{request(f,2)};
            let before=f.sharing.snapshot().await;
            let (source,w,release,started,_)=pool_gate::captured(f).await;
            let mut pending=Box::pin(source.read_session_file_chunk(&auth(f).target_token,&w,&r));let mut early=None;
            let committed=tokio::select! {result=&mut pending=>{early=Some(result);false},result=tokio::time::timeout(Duration::from_secs(3),started)=>result.is_ok_and(|r|r.is_ok())};
            let mut held=holder(f,&f.sharing.base.publication_schema,PUB_MARKER).await;let blocker=backend(&mut held).await;
            drop(release);
            let waited=if early.is_some(){false}else{tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes}};
            // The initial transaction is already committed; this new marker wait is in the
            // FINAL transaction, after private verified file bytes exist but before disclosure.
            let refused=source.read_session_file_chunk(&auth(f).target_token,&w,&r).await.err();
            let elapsed=if waited {if let Some(boundary)=boundary {arrived(f,boundary).await}else{expired(f).await}}else{false};
            held.rollback().await.unwrap();let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
            assert_eq!(f.sharing.snapshot().await,before);
            if session_expiry {refresh_expiry(f).await;}
            assert_eq!(read_as(f,&source,&auth(f).target_token,&w,&f.request()).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));
            assert!(committed&&waited&&elapsed,"initial acknowledged metadata and real later marker blocker must straddle DB expiry");
            assert_eq!(refused,Some(SessionFileChunkReadError::CapacityExhausted),"private completed bytes retain response capacity throughout final SQL wait");
            let error=if session_expiry {session(SessionCommandError::InvalidSession)}else{denied()};
            assert_eq!(result.err(),Some(error),"no byte DTO can escape an expired final window");drain(f).await;
        })).await;
    }
}
