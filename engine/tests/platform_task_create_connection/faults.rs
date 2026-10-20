use super::fixture::*;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_immediate_and_deferred_side_effects_never_confirm_a_task() {
    for mode in [
        "suppressed",
        "tampered",
        "session",
        "deferred_task",
        "deferred_session",
        "deferred_marker",
        "deferred_artifact_shape",
        "deferred_failure",
    ] {
        Fixture::checked(|f| Box::pin(async move {
            let p=Prepared::new(f).await;
            let connector=p.connector().await;
            let before=snapshot(f).await;
            let body=match mode {
                "suppressed"=>"RETURN NULL;".to_owned(),
                "tampered"=>format!("NEW.snapshot=pg_catalog.jsonb_set(NEW.snapshot::jsonb,'{{title}}','\"Injected\"')::text; NEW.manifest=pg_catalog.jsonb_set(NEW.manifest::jsonb,'{{title}}','\"Injected\"')::text; UPDATE {}.collaboration_tasks SET snapshot=NEW.snapshot,manifest=NEW.manifest WHERE task_id=NEW.task_id; RETURN NEW;",f.task),
                "deferred_task"=>format!("UPDATE {0}.collaboration_tasks SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred injection\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred injection\"')::text WHERE task_id=NEW.task_id; UPDATE {0}.collaboration_task_outbox SET snapshot=pg_catalog.jsonb_set(snapshot::jsonb,'{{title}}','\"Deferred injection\"')::text,manifest=pg_catalog.jsonb_set(manifest::jsonb,'{{title}}','\"Deferred injection\"')::text WHERE event_id=NEW.event_id; RETURN NEW;",f.task),
                "session"|"deferred_session"=>format!("DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",f.source,token_hash(&p.tokens[0])),
                "deferred_marker"=>format!("UPDATE {}.collaboration_artifact_schema SET workspace_id='{}'; RETURN NEW;",f.artifacts,uuid::Uuid::new_v4()),
                "deferred_artifact_shape"=>format!("ALTER TABLE {}.collaboration_artifacts ENABLE ROW LEVEL SECURITY; RETURN NEW;",f.artifacts),
                _=>"RAISE EXCEPTION 'synthetic deferred create failure';".to_owned(),
            };
            f.sql(&format!("CREATE SEQUENCE {}.platform_create_fault_hits",f.task)).await;
            // Owned disposable fixture only: fixed search_path and fully qualified side effects.
            f.sql(&format!("CREATE FUNCTION {0}.platform_create_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{0}.platform_create_fault_hits'); {body} END $$",f.task)).await;
            let (kind,timing)=if mode.starts_with("deferred") {("CONSTRAINT TRIGGER","AFTER INSERT") }else{("TRIGGER","BEFORE INSERT")};
            let deferred=if mode.starts_with("deferred") {"DEFERRABLE INITIALLY DEFERRED"}else{""};
            f.sql(&format!("CREATE {kind} platform_create_fault {timing} ON {0}.collaboration_task_outbox {deferred} FOR EACH ROW EXECUTE FUNCTION {0}.platform_create_fault()",f.task)).await;
            let result=p.create(&connector,0,task_id(10),"Only exact committed task can escape").await;
            let artifact_rls:bool=within(query("SELECT c.relrowsecurity AS rls FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname='collaboration_artifacts'")
                .bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("rls");
            assert!(result.is_err(),"{mode} must reject creation; Artifact RLS enabled={artifact_rls}");
            assert!(!artifact_rls,"{mode} must not commit changed Artifact business-table shape");
            let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.platform_create_fault_hits",f.task)).fetch_one(f.owner())).await.unwrap().get("hits");
            assert_eq!(hits,1,"{mode} fault must actually execute");
            assert_eq!(before,snapshot(f).await,"{mode} must leave no mutated task, Session or marker");
            assert_eq!(p.count("collaboration_tasks").await,0);
            assert_eq!(p.count("collaboration_task_outbox").await,0);
            assert_eq!(std::fs::read_dir(p.root.path()).unwrap().count(),1);
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })).await;
    }
}
