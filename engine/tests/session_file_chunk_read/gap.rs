use crate::{fixture::Fixture, pool_gate, support::*, *};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_rejects_original_source_registry_recipient_and_publication_drift_across_gap(
) {
    for mode in [
        "grant",
        "publication",
        "source_key",
        "root_key",
        "journal_key",
        "source",
        "registry",
        "recipientrelation",
        "publicationrelation",
        "logout",
        "suspended",
    ] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            let before=f.sharing.snapshot().await;let r=f.request();
            let (source,w,release,started,_)=pool_gate::captured(f).await;
            let mut pending=Box::pin(source.read_session_file_chunk(&auth(f).target_token,&w,&r));
            let mut early=None;
            let committed=tokio::select! {result=&mut pending=>{early=Some(result);false},result=tokio::time::timeout(Duration::from_secs(3),started)=>result.is_ok_and(|r|r.is_ok())};
            // An independent exclusive source/target transaction is admitted while the test
            // pool retains the initial committed connection. NO initial SQL fence may survive.
            let mut writer=auth(f).base.admin.begin().await.unwrap();
            let target_table=if mode.starts_with("recipient")||mode=="grant" {format!("{}.{HEADS}",f.sharing.schema)}
                else if mode.starts_with("publication")||["source_key","root_key","journal_key"].contains(&mode) {format!("{}.{PUB_ROWS}",f.sharing.base.publication_schema)}
                else {format!("{}.users",auth(f).base.schema)};
            let unlocked=tokio::time::timeout(Duration::from_secs(2),query(&format!("LOCK TABLE {target_table} IN ACCESS EXCLUSIVE MODE")).execute(&mut *writer)).await;
            assert!(unlocked.is_ok(),"the original metadata transaction cannot retain its {mode} relation lock over file work");unlocked.unwrap().unwrap();
            let mut restore_table=None;let mut restore_nonce=None;let mut restore_key=None;
            let error=match mode {
                "grant"=>{
                    // Drop the probe transaction before a legitimate separate full-ledger writer.
                    writer.rollback().await.unwrap();
                    f.sharing.store.revoke(r.download.grant_id,f.sharing.actor(),sharing_fixture::revision(1)).await.unwrap();
                    writer=auth(f).base.admin.begin().await.unwrap();recipient(FileRecipientGrantStoreError::InvalidRecord)
                },
                "publication"=>{
                    let old:Uuid=query(&format!("SELECT nonce FROM {}.{PUB_ROWS} WHERE upload_id=$1",f.sharing.base.publication_schema)).bind(r.download.upload_id.as_uuid()).fetch_one(&mut *writer).await.unwrap().get("nonce");restore_nonce=Some(old);
                    query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(Uuid::new_v4()).bind(r.download.upload_id.as_uuid()).execute(&mut *writer).await.unwrap();publication(FileUploadPublicationStoreError::InvalidRecord)
                },
                "source_key"|"root_key"|"journal_key"=>{
                    let old:Vec<u8>=query(&format!("SELECT {mode} AS k FROM {}.{PUB_ROWS} WHERE upload_id=$1",f.sharing.base.publication_schema))
                        .bind(r.download.upload_id.as_uuid()).fetch_one(&mut *writer).await.unwrap().get("k");
                    assert_eq!(old.len(),32);let mut changed=old.clone();changed[0]^=1;
                    query(&format!("UPDATE {}.{PUB_ROWS} SET {mode}=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(changed).bind(r.download.upload_id.as_uuid()).execute(&mut *writer).await.unwrap();
                    restore_key=Some((mode,old));publication(FileUploadPublicationStoreError::InvalidRecord)
                },
                "source"|"registry"|"recipientrelation"|"publicationrelation"=>{
                    let (schema,table,marker)=match mode {
                        "source"=>(auth(f).base.schema.clone(),"users",None),
                        "registry"=>(auth(f).base.schema.clone(),"collaboration_principals",None),
                        "recipientrelation"=>(f.sharing.schema.clone(),CHANGES,Some((MARKER,"changes_oid"))),
                        _=>(f.sharing.base.publication_schema.clone(),PUB_ROWS,Some((PUB_MARKER,"publications_oid"))),
                    };
                    for statement in [format!("ALTER TABLE {schema}.{table} RENAME TO original_{table}"),
                        format!("CREATE TABLE {schema}.{table}(LIKE {schema}.original_{table} INCLUDING ALL)"),
                        format!("INSERT INTO {schema}.{table} SELECT * FROM {schema}.original_{table}")] {query(&statement).execute(&mut *writer).await.unwrap();}
                    if let Some((marker,column))=marker {query(&format!("UPDATE {schema}.{marker} SET {column}='{schema}.{table}'::regclass::oid::bigint")).execute(&mut *writer).await.unwrap();}
                    restore_table=Some((schema,table,marker));
                    if mode=="recipientrelation" {recipient(FileRecipientGrantStoreError::UnsupportedSchema)}
                    else if mode=="publicationrelation" {publication(FileUploadPublicationStoreError::UnsupportedSchema)}
                    else{session(SessionCommandError::Authentication(DurableAuthError::SourceMismatch))}
                },
                "logout"=>{writer.rollback().await.unwrap();auth(f).source.logout(&auth(f).target_token).await.unwrap();writer=auth(f).base.admin.begin().await.unwrap();session(SessionCommandError::InvalidSession)},
                _=>{writer.rollback().await.unwrap();membership(f,f.sharing.recipient.principal().principal_id,MembershipStatus::Suspended).await;writer=auth(f).base.admin.begin().await.unwrap();identity(IdentityStoreError::AccessDenied)},
            };
            let injected=snapshot_in_tx(f,&mut writer).await;writer.commit().await.unwrap();drop(release);
            let result=match early {Some(v)=>v,None=>pending.as_mut().await};drop(pending);
            assert_eq!(result.err(),Some(error),"original {mode} must not be recaptured after the gap");assert_eq!(f.sharing.snapshot().await,injected);
            if let Some((schema,table,marker))=restore_table {
                let mut restore=auth(f).base.admin.begin().await.unwrap();
                query(&format!("DROP TABLE {schema}.{table}")).execute(&mut *restore).await.unwrap();
                query(&format!("ALTER TABLE {schema}.original_{table} RENAME TO {table}")).execute(&mut *restore).await.unwrap();
                if let Some((marker,column))=marker {query(&format!("UPDATE {schema}.{marker} SET {column}='{schema}.{table}'::regclass::oid::bigint")).execute(&mut *restore).await.unwrap();}
                restore.commit().await.unwrap();assert_eq!(f.sharing.snapshot().await,before);healthy(f).await;
            }else if let Some(nonce)=restore_nonce {
                query(&format!("UPDATE {}.{PUB_ROWS} SET nonce=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(nonce).bind(r.download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();assert_eq!(f.sharing.snapshot().await,before);healthy(f).await;
            }else if let Some((column,key))=restore_key {
                query(&format!("UPDATE {}.{PUB_ROWS} SET {column}=$1 WHERE upload_id=$2",f.sharing.base.publication_schema)).bind(key).bind(r.download.upload_id.as_uuid()).execute(&auth(f).base.admin).await.unwrap();assert_eq!(f.sharing.snapshot().await,before);healthy(f).await;
            }else if mode=="suspended" {membership(f,f.sharing.recipient.principal().principal_id,MembershipStatus::Active).await;healthy(f).await;}
            else if mode=="grant" {own_grant(f,2,grant_fixture::validity()).await;assert_eq!(read(f,&request(f,2)).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));}
            else {let login=auth(f).base.seed_session(&auth(f).target).await;assert_eq!(read_as(f,&auth(f).source,&login.token,&f.workspace,&f.request()).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));}
            assert!(committed,"after_release must observe the actual first COMMIT, not a timer");drain(f).await;
        })).await;
    }
    // The synthetic pool callback proves a real commit/gap, not entry into a file worker.
    // Separate private filesystem unit tests establish actual in-job drift/cancellation.
}
