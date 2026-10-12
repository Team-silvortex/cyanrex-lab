use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_final_clock_rechecks_session_and_grant_windows_after_real_wait() {
    checked(Fixture::ready().await,|f|Box::pin(async move {
        expire_soon(f).await;let before=f.sharing.snapshot().await;
        let mut held=pub_holder(f).await;let blocker=backend(&mut held).await;
        let r=f.request();let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
        let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
        let elapsed=waited&&expired(f).await;held.rollback().await.unwrap();
        let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
        assert_eq!(f.sharing.snapshot().await,before);
        refresh_expiry(f).await;healthy(f).await;drain(f).await;
        assert!(waited&&elapsed,"a grant-selected real Published marker wait must outlive the DB Session deadline");
        assert_eq!(result,Err(session(SessionCommandError::InvalidSession)));
    })).await;
    for arrival in [false, true] {
        checked(Fixture::ready().await,move |f|Box::pin(async move {
            let boundary=db_now(f).await+2;
            let window=if arrival {FileGrantValidity::new(boundary,MAX_FILE_GRANT_UNIX_SECONDS).unwrap()}
                else{FileGrantValidity::new(0,boundary).unwrap()};
            own_grant(f,2,window).await;
            let before=f.sharing.snapshot().await;let initially_before=db_now(f).await<boundary;
            let mut held=pub_holder(f).await;let blocker=backend(&mut held).await;
            let r=request(f,2);let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
            let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
            let elapsed=waited&&arrived(f,boundary).await;held.rollback().await.unwrap();
            let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
            assert_eq!(f.sharing.snapshot().await,before);
            assert!(initially_before&&waited&&elapsed,"real selected Published wait, not lock-timeout or an already elapsed window");
            if arrival {assert_eq!(result.unwrap(),f.sharing.base.manifest,
                "not_before may arrive during a genuine wait because only the FINAL sample decides this manifest-only observation");}
            else{assert_eq!(result,Err(SessionFileDownloadError::PermissionDenied),
                "an initially current recipient window cannot survive its final half-open upper boundary");}
            healthy(f).await;drain(f).await;
        })).await;
    }
}
