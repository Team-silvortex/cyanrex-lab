use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::ArtifactRevision,
    services::{
        artifact_store::ArtifactStoreError, auth_service::durable_source::SessionArtifactError,
    },
};
use sqlx_core::{query::query, row::Row};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};

fn valid_advance(p: &Prepared<'_>, seed: &ArtifactRevision) -> String {
    let bytes = b"Only exact publication may escape";
    let first = p.reference(2, bytes);
    let mut next = seed.clone();
    next.reference = first.clone();
    next.reference.revision_id = revision_id(222);
    next.sequence = 2.try_into().unwrap();
    next.parent = Some(first);
    next.byte_length = bytes.len() as u64;
    let raw = serde_json::to_string(&next).unwrap();
    let decoded: ArtifactRevision = serde_json::from_str(&raw).unwrap();
    assert_eq!(decoded, next);
    assert_eq!(u64::from(decoded.sequence), 2);
    assert_eq!(decoded.parent, Some(p.reference(2, bytes)));
    let path = p.blob(&next.reference);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o400))
        .unwrap();
    file.sync_all().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().nlink(), 1);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    format!("INSERT INTO {0}.collaboration_artifact_revisions(revision_id,artifact_id,sequence,snapshot) VALUES('{1}',NEW.artifact_id,2,$snapshot${raw}$snapshot$); INSERT INTO {0}.collaboration_artifact_outbox(event_id,artifact_id,revision_id,sequence,actor_id,event_type,snapshot) VALUES('{2}',NEW.artifact_id,'{1}',2,NEW.actor_id,'cyanrex.artifact.revised',$snapshot${raw}$snapshot$); UPDATE {0}.collaboration_artifacts SET head_revision_id='{1}',sequence=2 WHERE artifact_id=NEW.artifact_id; IF NOT EXISTS(SELECT 1 FROM {0}.collaboration_artifacts WHERE artifact_id=NEW.artifact_id AND head_revision_id='{1}' AND sequence=2) THEN RAISE EXCEPTION 'synthetic valid-v2 head advance was not completed'; END IF; PERFORM pg_catalog.nextval('{0}.text_valid_head_done'); RETURN NEW;",
        p.f.artifacts,next.reference.revision_id,uuid::Uuid::new_v4())
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_immediate_and_deferred_publication_faults_never_confirm() {
    for mode in [
        "suppressed_revision",
        "suppressed_event",
        "tampered",
        "deferred_record",
        "deferred_valid_head",
        "deferred_shape",
        "deferred_session",
        "deferred_marker",
        "deferred_failure",
    ] {
        Fixture::checked(|f| Box::pin(async move {
            let p=Prepared::new(f).await; let connector=p.connector().await;
            let original=p.create(&connector,0,1,"Preserve committed original").await.unwrap();
            let before=snapshot(f).await;
            let body=match mode {
                "suppressed_revision"|"suppressed_event"=>"RETURN NULL;".to_owned(),
                "tampered"|"deferred_record"=>format!("UPDATE {0}.collaboration_artifact_revisions SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Injected\"')::text WHERE artifact_id=NEW.artifact_id; UPDATE {0}.collaboration_artifact_outbox SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Injected\"')::text WHERE event_id=NEW.event_id; NEW.snapshot=pg_catalog.jsonb_set(NEW.snapshot::jsonb,'{{title}}','\"Injected\"')::text; RETURN NEW;",f.artifacts),
                "deferred_valid_head"=>valid_advance(&p,&original),
                "deferred_shape"=>format!("ALTER TABLE {}.collaboration_artifacts ENABLE ROW LEVEL SECURITY; RETURN NEW;",f.artifacts),
                "deferred_session"=>format!("DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",f.source,token_hash(&p.tokens[0])),
                "deferred_marker"=>format!("UPDATE {}.collaboration_task_schema SET workspace_id='{}'; RETURN NEW;",f.task,uuid::Uuid::new_v4()),
                _=>"RAISE EXCEPTION 'synthetic text publication failure';".to_owned(),
            };
            // The valid-v2 fault pre-creates its immutable bytes; they too must survive failure.
            let files=p.files();
            f.sql(&format!("CREATE SEQUENCE {}.text_fault_hits",f.artifacts)).await;
            if mode=="deferred_valid_head" {
                f.sql(&format!("CREATE SEQUENCE {}.text_valid_head_done",f.artifacts)).await;
            }
            f.sql(&format!("CREATE FUNCTION {0}.text_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN IF NEW.sequence<>1 THEN RETURN NEW; END IF; PERFORM pg_catalog.nextval('{0}.text_fault_hits'); {body} END $$",f.artifacts)).await;
            let table=if mode=="suppressed_revision" {"collaboration_artifact_revisions"} else {"collaboration_artifact_outbox"};
            let deferred=mode.starts_with("deferred");
            f.sql(&format!("CREATE {} text_fault {} ON {}.{table} {} FOR EACH ROW EXECUTE FUNCTION {}.text_fault()",
                if deferred {"CONSTRAINT TRIGGER"}else{"TRIGGER"},if deferred{"AFTER INSERT"}else{"BEFORE INSERT"},f.artifacts,
                if deferred{"DEFERRABLE INITIALLY DEFERRED"}else{""},f.artifacts)).await;
            let result=p.create(&connector,0,2,"Only exact publication may escape").await;
            if mode=="deferred_valid_head" {
                assert_eq!(result,Err(SessionArtifactError::Artifact(ArtifactStoreError::InvalidRecord)),"a completed legal head advance must fail current-head confirmation, not trigger SQL or blob work");
            } else {assert!(result.is_err(),"{mode}");}
            assert!(super::storage_helpers::no_connections(f).await);
            let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.text_fault_hits",f.artifacts))
                .fetch_one(f.owner())).await.unwrap().get("hits");
            assert_eq!(hits,1,"{mode} fault must actually execute");
            if mode=="deferred_valid_head" {
                let done:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS done FROM {}.text_valid_head_done",f.artifacts))
                    .fetch_one(f.owner())).await.unwrap().get("done");
                assert_eq!(done,1,"valid-v2 INSERTs and exact head advance must complete before rejection");
            }
            let rls:bool=within(query("SELECT c.relrowsecurity AS rls FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname='collaboration_artifacts'")
                .bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("rls");
            assert!(!rls,"deferred metadata shape may not commit"); assert_eq!(before,snapshot(f).await); p.counts(1).await;
            p.old_files_unchanged(&files,usize::from(mode!="suppressed_revision"));
            if mode!="suppressed_revision" {
                assert_eq!(std::fs::read(p.blob(&p.reference(2,b"Only exact publication may escape"))).unwrap(),b"Only exact publication may escape");
            }
            p.read_committed(&original,b"Preserve committed original").await;
            drop(connector); assert!(p.finish());
        })).await;
    }
}
