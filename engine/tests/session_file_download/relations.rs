use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_retains_source_registry_and_target_relation_fences() {
    for mode in ["source", "registry", "publication", "recipient"] {
        checked(Fixture::ready().await,move |f|Box::pin(async move {
            let before=f.sharing.snapshot().await;
            let mut held=pub_holder(f).await;let blocker=backend(&mut held).await;
            let r=f.request();let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
            let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
            let reader=reader_pid(f,blocker).await;
            let Some(reader)=reader else {
                held.rollback().await.unwrap();let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
                panic!("metadata reader did not reach the real final-target blocker: {result:?}");
            };
            let (schema,table,marker_column)=match mode {
                "source"=>(auth(f).base.schema.clone(),"users",None),
                "registry"=>(auth(f).base.schema.clone(),"collaboration_principals",None),
                "publication"=>(f.sharing.base.publication_schema.clone(),PUB_ROWS,Some((PUB_MARKER,"publications_oid"))),
                _=>(f.sharing.schema.clone(),CHANGES,Some((MARKER,"changes_oid"))),
            };
            // A later AccessShare snapshot would queue behind the pending exclusive DDL
            // writer and prevent this test from releasing its own reader blocker.
            let during=f.sharing.snapshot().await;
            let mut writer=auth(f).base.admin.begin().await.unwrap();let writer_pid=backend(&mut writer).await;
            let lock=format!("LOCK TABLE {schema}.{table} IN ACCESS EXCLUSIVE MODE");
            let mut replacement=Box::pin(query(&lock).execute(&mut *writer));let mut early_writer=None;
            let writer_waited=tokio::select! {result=&mut replacement=>{early_writer=Some(result);false},yes=pid_blocked(f,writer_pid,reader)=>yes};
            held.rollback().await.unwrap();
            // Poll both real backends after releasing the test holder: the original reader's
            // AccessShare/SHARE relation pins must finish BEFORE DDL can acquire its lock.
            let (observed,locked)=tokio::join!(async {match early {Some(v)=>v,None=>pending.as_mut().await}},
                async {match early_writer {Some(v)=>v,None=>replacement.as_mut().await}});
            drop(pending);drop(replacement);locked.unwrap();
            for statement in [format!("ALTER TABLE {schema}.{table} RENAME TO original_{table}"),
                format!("CREATE TABLE {schema}.{table}(LIKE {schema}.original_{table} INCLUDING ALL)"),
                format!("INSERT INTO {schema}.{table} SELECT * FROM {schema}.original_{table}")] {
                query(&statement).execute(&mut *writer).await.unwrap();
            }
            if let Some((marker,column))=marker_column {query(&format!("UPDATE {schema}.{marker} SET {column}='{schema}.{table}'::regclass::oid::bigint"))
                .execute(&mut *writer).await.unwrap();}
            let injected=snapshot_in_tx(f,&mut writer).await;writer.commit().await.unwrap();
            assert_eq!(f.sharing.snapshot().await,injected);
            assert!(waited&&writer_waited,"the exact reader must fence {mode} replacement, not just delay a task");
            assert_eq!(during,before);assert_eq!(observed.unwrap(),f.sharing.base.manifest);
            // LIKE is only a writer-side replacement probe, not a full installation: it does
            // not recreate source immutability triggers or retarget the original Session FKs.
            // Restore the ORIGINAL physical table and configured OIDs before the health control.
            let mut restore=auth(f).base.admin.begin().await.unwrap();
            query(&format!("DROP TABLE {schema}.{table}")).execute(&mut *restore).await.unwrap();
            query(&format!("ALTER TABLE {schema}.original_{table} RENAME TO {table}")).execute(&mut *restore).await.unwrap();
            if let Some((marker,column))=marker_column {query(&format!("UPDATE {schema}.{marker} SET {column}='{schema}.{table}'::regclass::oid::bigint"))
                .execute(&mut *restore).await.unwrap();}
            restore.commit().await.unwrap();assert_eq!(f.sharing.snapshot().await,before);
            healthy(f).await;drain(f).await;
        })).await;
    }
    checked(Fixture::ready().await,|f|Box::pin(async move {
        // Capture the original configured publication marker before its row wait. A valid
        // installation change made by the holder must be freshly checked, not recaptured.
        let original:Uuid=query(&format!("SELECT installation_id FROM {}.{PUB_MARKER}",f.sharing.base.publication_schema))
            .fetch_one(&auth(f).base.admin).await.unwrap().get("installation_id");
        let mut held=holder(f,&f.sharing.base.publication_schema,PUB_MARKER,true).await;let blocker=backend(&mut held).await;
        let r=f.request();let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
        let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
        query(&format!("UPDATE {}.{PUB_MARKER} SET installation_id=$1",f.sharing.base.publication_schema)).bind(Uuid::new_v4()).execute(&mut *held).await.unwrap();
        let injected=snapshot_in_tx(f,&mut held).await;held.commit().await.unwrap();
        let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
        assert_eq!(f.sharing.snapshot().await,injected);
        query(&format!("UPDATE {}.{PUB_MARKER} SET installation_id=$1",f.sharing.base.publication_schema)).bind(original).execute(&auth(f).base.admin).await.unwrap();
        healthy(f).await;assert!(waited);assert_eq!(result,Err(publication(FileUploadPublicationStoreError::IncarnationMismatch)));drain(f).await;
    })).await;
}
