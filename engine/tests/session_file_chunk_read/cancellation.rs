use crate::{fixture::Fixture, pool_gate, support::*, *};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_cancelled_final_wait_releases_private_buffer_and_sql_locks_without_writes(
) {
    for table in [MARKER, HEADS, CHANGES, PUB_MARKER, PUB_ROWS] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            let r=f.request();let before=f.sharing.snapshot().await;
            let (source,w,release,started,_)=pool_gate::captured(f).await;
            let mut pending=Box::pin(source.read_session_file_chunk(&auth(f).target_token,&w,&r));let mut early=None;
            let committed=tokio::select! {result=&mut pending=>{early=Some(result);false},result=tokio::time::timeout(Duration::from_secs(3),started)=>result.is_ok_and(|r|r.is_ok())};
            let schema=if [PUB_MARKER,PUB_ROWS].contains(&table) {&f.sharing.base.publication_schema}else{&f.sharing.schema};
            let mut held=holder(f,schema,table).await;let blocker=backend(&mut held).await;drop(release);
            let waited=if early.is_some(){false}else{tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes}};
            let refused=source.read_session_file_chunk(&auth(f).target_token,&w,&r).await.err();
            drop(pending);held.rollback().await.unwrap();drain(f).await;
            assert_eq!(f.sharing.snapshot().await,before);
            assert_eq!(read_as(f,&source,&auth(f).target_token,&w,&r).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));
            assert!(committed&&waited&&early.is_none(),"cancel must occur at actual post-file final {table} row wait");
            assert_eq!(refused,Some(SessionFileChunkReadError::CapacityExhausted));
        })).await;
    }
    // Only an observed pre-COMMIT SELECT/row wait is cancelled. This proves private-result
    // capacity/SQL cleanup and absence of writes, not arbitrary lost ACK or network disclosure.
}
