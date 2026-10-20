use super::fixture::*;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_immediate_deferred_task_and_full_union_faults_never_commit()
{
    for mode in [
        "suppressed",
        "tampered",
        "deferred_task",
        "deferred_old",
        "deferred_new",
        "deferred_session",
        "deferred_artifact_shape",
        "deferred_failure",
    ] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;
            let old_artifact=p.artifact(0,b"Original retained immutable notes").await;
            let new_artifact=p.revise(&old_artifact,b"Next retained immutable notes").await;
            let old=p.task_manifest(10,0,manifest("Original Task",vec![old_artifact.reference.clone()])).await;
            let connector=p.connector().await;let before=snapshot(f).await;let files=p.files();
            let body=match mode {
                "suppressed"=>"RETURN NULL;".to_owned(),
                "tampered"=>format!("NEW.snapshot=pg_catalog.jsonb_set(NEW.snapshot::jsonb,'{{title}}','\"Injected\"')::text; NEW.manifest=pg_catalog.jsonb_set(NEW.manifest::jsonb,'{{title}}','\"Injected\"')::text; UPDATE {}.collaboration_tasks SET snapshot=NEW.snapshot,manifest=NEW.manifest WHERE task_id=NEW.task_id; RETURN NEW;",f.task),
                "deferred_task"=>format!("UPDATE {0}.collaboration_tasks SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred injection\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred injection\"')::text WHERE task_id=NEW.task_id; UPDATE {0}.collaboration_task_outbox SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred injection\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred injection\"')::text WHERE event_id=NEW.event_id; RETURN NEW;",f.task),
                "deferred_old"|"deferred_new"=>format!("UPDATE {}.collaboration_artifact_revisions SET snapshot='{{}}' WHERE revision_id='{}'; RETURN NEW;",f.artifacts,if mode=="deferred_old"{old_artifact.reference.revision_id}else{new_artifact.reference.revision_id}),
                "deferred_session"=>format!("DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",f.source,token_hash(&p.tokens[0])),
                "deferred_artifact_shape"=>format!("ALTER TABLE {}.collaboration_artifacts ENABLE ROW LEVEL SECURITY; RETURN NEW;",f.artifacts),
                _=>"RAISE EXCEPTION 'synthetic deferred replacement failure';".to_owned(),
            };
            f.sql(&format!("CREATE SEQUENCE {}.platform_replace_fault_hits",f.task)).await;
            f.sql(&format!("CREATE FUNCTION {0}.platform_replace_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{0}.platform_replace_fault_hits'); {body} END $$",f.task)).await;
            let deferred=mode.starts_with("deferred");
            f.sql(&format!("CREATE {} platform_replace_fault {} ON {}.collaboration_task_outbox {} FOR EACH ROW EXECUTE FUNCTION {}.platform_replace_fault()",
                if deferred{"CONSTRAINT TRIGGER"}else{"TRIGGER"},if deferred{"AFTER INSERT"}else{"BEFORE INSERT"},f.task,if deferred{"DEFERRABLE INITIALLY DEFERRED"}else{""},f.task)).await;
            // Removing the final old payload must still validate its original table and bytes.
            let requested=manifest("Requested exact edit",if mode=="deferred_artifact_shape"{vec![]}else{vec![new_artifact.reference.clone()]});
            let result=p.replace(&connector,0,&old,requested).await;
            let rls:bool=within(query("SELECT c.relrowsecurity AS rls FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname='collaboration_artifacts'").bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("rls");
            assert!(result.is_err(),"{mode} must reject replacement; Artifact RLS enabled={rls}");assert!(!rls);
            let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.platform_replace_fault_hits",f.task)).fetch_one(f.owner())).await.unwrap().get("hits");
            assert_eq!(hits,1,"{mode} must actually execute");assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());p.unchanged(&old,1).await;
            drop(connector);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
