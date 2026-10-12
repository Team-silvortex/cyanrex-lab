use crate::{fixture::Fixture, support::*};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_cancelled_real_wait_drains_without_metadata_or_file_writes() {
    for table in [MARKER, HEADS, CHANGES, PUB_MARKER, PUB_ROWS] {
        checked(Fixture::ready().await,move |f|Box::pin(async move {
            let schema=if [PUB_MARKER,PUB_ROWS].contains(&table) {&f.sharing.base.publication_schema}else{&f.sharing.schema};
            let before=f.sharing.snapshot().await;let mut held=holder(f,schema,table,false).await;let blocker=backend(&mut held).await;
            let r=f.request();let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
            let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
            drop(pending);held.rollback().await.unwrap();drain(f).await;
            let after=f.sharing.snapshot().await;healthy(f).await;
            assert!(waited&&early.is_none(),"cancellation must happen while the exact selected {table} really blocks the source-owned read");
            assert_eq!(after,before);
        })).await;
    }
    // These calls perform no application writes at all. Dropping during an independently
    // observed SELECT/row-lock wait proves cleanup and no metadata/file mutation here; it is
    // not lost-COMMIT acknowledgement coverage, durable download state or a retry policy.
}
