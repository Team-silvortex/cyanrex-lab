use super::fixture::*;
use cyanrex_engine::services::{
    artifact_store::ArtifactStoreError, auth_service::durable_source::SessionArtifactError,
};
use std::sync::Arc;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_duplicate_ids_concurrent_publication_and_orphans_never_adopt(
) {
    Fixture::checked(|f| Box::pin(async move {
        let p=Prepared::new(f).await; let connector=Arc::new(p.connector().await);
        let first=p.create(&connector,0,1,"Committed original").await.unwrap(); let files=p.files(); let before=snapshot(f).await;
        for owner in [0,1] { assert_eq!(p.create(&connector,owner,1,"Never overwrite").await,Err(SessionArtifactError::Artifact(ArtifactStoreError::Conflict))); }
        assert_eq!(connector.create_session_text_artifact(&p.tokens[0],artifact_id(2),revision_id(1),"No revision reuse".into(),"No adoption".into()).await,
            Err(SessionArtifactError::Artifact(ArtifactStoreError::Conflict)));
        assert_eq!(before,snapshot(f).await); assert_eq!(files,p.files());
        let (holder,pid)=super::waits::holder(f).await;
        let c=Arc::clone(&connector); let token=p.tokens[0].clone();
        let a=tokio::spawn(async move { c.create_session_text_artifact(&token,artifact_id(3),revision_id(3),"Race A".into(),"Exact race bytes".into()).await });
        let first_pid=super::waits::blocked(f,pid).await;
        let c=Arc::clone(&connector); let token=p.tokens[1].clone();
        let b=tokio::spawn(async move { c.create_session_text_artifact(&token,artifact_id(3),revision_id(4),"Race B".into(),"Cannot inherit A".into()).await });
        super::waits::blocked(f,first_pid).await;
        assert!(!a.is_finished()&&!b.is_finished()); super::waits::release(holder).await;
        assert!(a.await.unwrap().is_ok()); assert_eq!(b.await.unwrap(),Err(SessionArtifactError::Artifact(ArtifactStoreError::Conflict)));
        f.sql(&format!("DROP TRIGGER text_connection_pause ON {}.collaboration_artifact_outbox",f.artifacts)).await;
        let committed=snapshot(f).await; let old=p.files();
        f.sql(&format!("CREATE FUNCTION {0}.text_orphan() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN RETURN NULL; END $$",f.artifacts)).await;
        f.sql(&format!("CREATE TRIGGER text_orphan BEFORE INSERT ON {}.collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION {}.text_orphan()",f.artifacts,f.artifacts)).await;
        assert!(p.create(&connector,0,5,"Retained unpublished text").await.is_err());
        assert_eq!(committed,snapshot(f).await); p.old_files_unchanged(&old,1);
        f.sql(&format!("DROP TRIGGER text_orphan ON {}.collaboration_artifact_outbox",f.artifacts)).await;
        let retained=p.files();
        assert_eq!(p.create(&connector,0,5,"Retained unpublished text").await,Err(SessionArtifactError::Artifact(ArtifactStoreError::Conflict)));
        assert_eq!(committed,snapshot(f).await); assert_eq!(retained,p.files()); p.counts(2).await;
        p.read_committed(&first,b"Committed original").await;
        drop(connector); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
