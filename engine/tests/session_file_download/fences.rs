use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_fences_queued_revocation_and_history_append_until_final_commit() {
    checked(Fixture::ready().await,|f|Box::pin(async move {
        let before=f.sharing.snapshot().await;let r=f.request();
        let mut held=pub_holder(f).await;let blocker=backend(&mut held).await;
        let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
        let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
        let Some(reader)=reader_pid(f,blocker).await else {held.rollback().await.unwrap();drop(pending);panic!("grant-selected reader did not block at Published marker");};
        let actor=f.sharing.actor();
        let mut revocation=Box::pin(f.sharing.store.revoke(r.grant_id,actor,sharing_fixture::revision(1)));let mut early_writer=None;
        let writer_waited=tokio::select! {result=&mut revocation=>{early_writer=Some(result);false},yes=named_writer_blocked(f,&f.sharing.schema,reader)=>yes};
        let during=f.sharing.snapshot().await;held.rollback().await.unwrap();
        let (observed,revoked)=tokio::join!(async {match early {Some(v)=>v,None=>pending.as_mut().await}},
            async {match early_writer {Some(v)=>v,None=>revocation.as_mut().await}});
        drop(pending);drop(revocation);let after=f.sharing.snapshot().await;
        assert!(waited&&writer_waited,"the grant table/history fence must actually hold a distinct revocation backend");
        assert_eq!(during,before);assert_eq!(observed.unwrap(),f.sharing.base.manifest);
        let expected=FileRecipientGrant::new(r.grant_id,sharing_fixture::revision(2),f.issued.grant().file().clone(),
            f.issued.grant().publisher(),f.issued.grant().recipient(),f.issued.grant().validity(),FileGrantState::Revoked).unwrap();
        let revoked=revoked.unwrap();assert_eq!(revoked.grant(),&expected);assert_eq!(revoked.issuer(),f.issued.issuer());
        sharing_fixture::recipient_delta(&f.sharing.schema,&before,&after,1,true);
        sharing_fixture::assert_record_rows(&after,&f.sharing.schema,&expected,f.issued.issuer(),actor);
        assert_eq!(read(f,&r).await,Err(SessionFileDownloadError::PermissionDenied));
        own_grant(f,2,grant_fixture::validity()).await;assert_eq!(read(f,&request(f,2)).await.unwrap(),f.sharing.base.manifest);drain(f).await;
    })).await;
    checked(Fixture::ready().await,|f|Box::pin(async move {
        let before=f.sharing.snapshot().await;let r=f.request();
        let mut held=pub_holder(f).await;let blocker=backend(&mut held).await;
        let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
        let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
        let Some(reader)=reader_pid(f,blocker).await else {held.rollback().await.unwrap();drop(pending);panic!("history-selected reader did not block at Published marker");};
        let mut writer=auth(f).base.admin.begin().await.unwrap();let writer_pid=backend(&mut writer).await;
        let append_sql=format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,2,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=1",f.sharing.schema,f.sharing.schema);
        let mut append=Box::pin(query(&append_sql).bind(r.grant_id.as_uuid()).execute(&mut *writer));let mut early_writer=None;
        let writer_waited=tokio::select! {result=&mut append=>{early_writer=Some(result);false},yes=pid_blocked(f,writer_pid,reader)=>yes};
        let during=f.sharing.snapshot().await;held.rollback().await.unwrap();
        let (observed,appended)=tokio::join!(async {match early {Some(v)=>v,None=>pending.as_mut().await}},
            async {match early_writer {Some(v)=>v,None=>append.as_mut().await}});
        drop(pending);drop(append);assert_eq!(appended.unwrap().rows_affected(),1);
        let injected=snapshot_in_tx(f,&mut writer).await;writer.commit().await.unwrap();
        assert_eq!(f.sharing.snapshot().await,injected);
        assert!(waited&&writer_waited,"missing revision append must wait for the exact live reader");
        assert_eq!(during,before);assert_eq!(observed.unwrap(),f.sharing.base.manifest);
        assert_eq!(read(f,&r).await,Err(recipient(FileRecipientGrantStoreError::InvalidRecord)));
        query(&format!("DELETE FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=2",f.sharing.schema)).bind(r.grant_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();
        assert_eq!(f.sharing.snapshot().await,before);healthy(f).await;drain(f).await;
    })).await;
    checked(Fixture::ready().await,|f|Box::pin(async move {
        // A legal revocation that commits while initial schema acquisition is waiting must
        // be seen freshly despite a configured REPEATABLE READ source-pool default.
        configure(f,false).await;let mut held=auth(f).base.admin.begin().await.unwrap();
        query(&format!("LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",f.sharing.schema)).execute(&mut *held).await.unwrap();
        let blocker=backend(&mut held).await;let r=f.request();let mut pending=Box::pin(read_direct(f,&r));let mut early=None;
        let waited=tokio::select! {result=&mut pending=>{early=Some(result);false},yes=blocked(f,blocker)=>yes};
        let old=f.issued.grant();let changed=FileRecipientGrant::new(old.id(),sharing_fixture::revision(2),old.file().clone(),old.publisher(),old.recipient(),old.validity(),FileGrantState::Revoked).unwrap();
        let raw=serde_json::to_vec(&changed).unwrap();
        query(&format!("UPDATE {}.{HEADS} SET revision=2,grant_payload=$1 WHERE grant_id=$2",f.sharing.schema)).bind(&raw).bind(old.id().as_uuid()).execute(&mut *held).await.unwrap();
        query(&format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,revision,issuer_id,issuer_account_id,not_before,expires_at,floor(extract(epoch FROM clock_timestamp()))::bigint,grant_payload FROM {}.{HEADS} WHERE grant_id=$1",f.sharing.schema,f.sharing.schema))
            .bind(old.id().as_uuid()).execute(&mut *held).await.unwrap();
        let injected=snapshot_in_tx(f,&mut held).await;held.commit().await.unwrap();
        let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
        assert!(waited);assert_eq!(result,Err(SessionFileDownloadError::PermissionDenied));assert_eq!(f.sharing.snapshot().await,injected);
        settings(f,false).await;own_grant(f,2,grant_fixture::validity()).await;
        assert_eq!(read(f,&request(f,2)).await.unwrap(),f.sharing.base.manifest);settings(f,false).await;drain(f).await;
    })).await;
}
