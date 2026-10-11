use super::{bytes::*, control::*, fixture::*, support::*};
use cyanrex_engine::services::{
    file_upload_chunk_journal::{FileUploadChunkJournalError, PersistentChunkState},
    file_upload_staging::{JournaledUploadChunkError, UploadChunkState},
};
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_unknown_insert_and_final_confirmation_faults_never_adopt_or_release() {
    for (body, deferred, expected) in [
        (
            "RETURN NULL;",
            false,
            FileUploadChunkJournalError::StorageUnavailable,
        ),
        (
            "RAISE EXCEPTION 'intentional insertion fault';",
            false,
            FileUploadChunkJournalError::StorageUnavailable,
        ),
        (
            "RAISE EXCEPTION 'intentional deferred insertion fault';",
            true,
            FileUploadChunkJournalError::StorageUnavailable,
        ),
        (
            "NEW.owner_id='00000000-0000-0000-0000-000000000123'::uuid;",
            false,
            FileUploadChunkJournalError::InvalidRecord,
        ),
    ] {
        let fixture = Fixture::ready(17).await;
        let source = fixture.rows(false).await;
        let before = fixture.rows(true).await;
        let oids = fixture.oids(true).await;
        fault(&fixture, "INSERT", body, deferred).await;
        let mut attempt = fixture
            .journaled
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(
            matches!(fixture.journaled.stage_chunk(&mut attempt,reader).await,Err(JournaledUploadChunkError::Journal(error)) if error==expected)
        );
        assert!(hit(&fixture).await);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        assert_eq!(fixture.rows(true).await, before);
        assert_eq!(fixture.rows(false).await, source);
        assert_eq!(fixture.oids(true).await, oids);
        assert!(tree(&fixture.root).is_empty());
        remove_fault(&fixture).await;
        // Known transaction fault is not a claimed lost-COMMIT acknowledgement or power-loss test.
        healthy(&fixture, 2).await;
        fixture.pool_restored(true).await;
        fixture.cleanup().await;
    }
    for (body,deferred) in [
        ("RETURN NULL;".to_owned(),false),
        ("RAISE EXCEPTION 'intentional final update fault';".to_owned(),false),
        ("RAISE EXCEPTION 'intentional deferred final fault';".to_owned(),true),
        // Exact full-ledger delta also rejects a trigger that changes an unrelated valid row.
        (format!("IF NEW.upload_id='{}'::uuid THEN UPDATE collaboration_file_upload_chunks SET confirmed=FALSE WHERE upload_id='{}'::uuid; END IF;",upload(1).as_uuid(),upload(2).as_uuid()),false),
    ] {
        let fixture=Fixture::ready(17).await;healthy(&fixture,2).await;
        let source=fixture.rows(false).await;let marker=fixture.rows(true).await[0].clone();let oids=fixture.oids(true).await;
        fault(&fixture,"UPDATE",&body,deferred).await;
        let mut attempt=fixture.journaled.prepare(upload(1),owner(1),account(1),0).await.unwrap();
        let (reader,release,started,finished)=held(0,17,0);let mut pending=Box::pin(fixture.journaled.stage_chunk(&mut attempt,reader));
        tokio::select! {result=&mut pending=>panic!("no actual input barrier: {result:?}"),_=signal(started)=>{}}
        let unknown=fixture.rows(true).await;let partial=tree(&fixture.root);assert_eq!(unknown[0],marker);
        metadata(&fixture.journal.get(upload(1),owner(1),account(1),0).await.unwrap().unwrap(),0,&fixture.manifest,PersistentChunkState::Unknown);
        assert_eq!(fixture.rows(true).await,unknown);assert_eq!(tree(&fixture.root),partial);
        drop(release);assert!(matches!(pending.await,Err(JournaledUploadChunkError::Unconfirmed)));signal(finished).await;
        assert!(hit(&fixture).await);assert_eq!(attempt.state(),UploadChunkState::Unknown);
        assert_eq!(fixture.rows(true).await,unknown);assert_eq!(fixture.rows(false).await,source);assert_eq!(fixture.oids(true).await,oids);
        let retained=tree(&fixture.root);assert_eq!(retained.len(),2);assert!(retained.iter().all(|entry|entry.length==17));
        for completed in partial.iter().filter(|entry|entry.length==17) {assert!(retained.contains(completed),"failure changed an unrelated healthy inode/hash/mode");}
        remove_fault(&fixture).await;
        let before=fixture.rows(true).await;
        metadata(&fixture.journal.get(upload(1),owner(1),account(1),0).await.unwrap().unwrap(),0,&fixture.manifest,PersistentChunkState::Unknown);
        assert_eq!(fixture.rows(true).await,before);assert_eq!(tree(&fixture.root),retained);
        let mut repeat=fixture.journaled.prepare(upload(1),owner(1),account(1),0).await.unwrap();let reader=Pattern::new(0,17);let calls=reader.calls.clone();
        assert!(matches!(fixture.journaled.stage_chunk(&mut repeat,reader).await,Err(JournaledUploadChunkError::Journal(FileUploadChunkJournalError::Conflict))));
        assert_eq!(calls.load(Ordering::SeqCst),0);assert_eq!(fixture.rows(true).await,before);assert_eq!(tree(&fixture.root),retained);
        healthy(&fixture,3).await;fixture.pool_restored(false).await;fixture.pool_restored(true).await;fixture.cleanup().await;
    }
}
