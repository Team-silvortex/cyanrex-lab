use super::fixture::*;
use axum::http::StatusCode;
use cyanrex_engine::models::collaboration::ArtifactRevision;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_http_deferred_faults_are_unconfirmed_and_orphans_never_adopt(
) {
    for mode in [
        "suppressed_event",
        "deferred_failure",
        "deferred_session",
        "deferred_shape",
    ] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;let route=router(&p,1).await;
            let value=call(&route,request(&p,0,"POST",PATH,wire(1,"Committed original","Preserve exact original")),StatusCode::CREATED).await;
            let original:ArtifactRevision=serde_json::from_value(value["artifact"].clone()).unwrap();
            let before=snapshot(f).await;let files=p.files();
            f.sql(&format!("CREATE SEQUENCE {}.text_http_fault_hits",f.artifacts)).await;
            let body=match mode {
                "suppressed_event"=>"RETURN NULL;".to_owned(),
                "deferred_session"=>format!("DELETE FROM {}.sessions WHERE token='{}'; RETURN NEW;",f.source,token_hash(&p.tokens[0])),
                "deferred_shape"=>format!("ALTER TABLE {}.collaboration_artifacts ENABLE ROW LEVEL SECURITY; RETURN NEW;",f.artifacts),
                _=>"RAISE EXCEPTION 'synthetic private SQL failure must not escape HTTP';".to_owned(),
            };
            f.sql(&format!("CREATE FUNCTION {0}.text_http_fault() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.nextval('{0}.text_http_fault_hits'); {body} END $$",f.artifacts)).await;
            let deferred=mode.starts_with("deferred");
            f.sql(&format!("CREATE {} text_http_fault {} ON {}.collaboration_artifact_outbox {} FOR EACH ROW EXECUTE FUNCTION {}.text_http_fault()",if deferred {"CONSTRAINT TRIGGER"} else {"TRIGGER"},if deferred {"AFTER INSERT"} else {"BEFORE INSERT"},f.artifacts,if deferred {"DEFERRABLE INITIALLY DEFERRED"} else {""},f.artifacts)).await;
            let (status,code)=if mode=="deferred_session" {(StatusCode::UNAUTHORIZED,"invalid_session")} else {(StatusCode::SERVICE_UNAVAILABLE,"service_unavailable")};
            error(&route,request(&p,0,"POST",PATH,wire(2,"Unconfirmed text","Retain these unpublished immutable bytes")),status,code,"unconfirmed").await;
            assert!(super::storage_helpers::no_connections(f).await);
            let hits:i64=within(query(&format!("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS hits FROM {}.text_http_fault_hits",f.artifacts)).fetch_one(f.owner())).await.unwrap().get("hits");
            assert_eq!(hits,1,"fault body must execute rather than fail preflight");
            // This is an observed pre-commit abort, not a lost-COMMIT-ack simulation.
            assert_eq!(before,snapshot(f).await);p.counts(1).await;p.old_files_unchanged(&files,1);
            let orphan=p.blob(&p.reference(2,b"Retain these unpublished immutable bytes"));
            assert_eq!(std::fs::read(&orphan).unwrap(),b"Retain these unpublished immutable bytes");
            let rls:bool=within(query("SELECT c.relrowsecurity AS rls FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname='collaboration_artifacts'").bind(&f.artifacts).fetch_one(f.owner())).await.unwrap().get("rls");
            assert!(!rls,"deferred schema mutation must roll back");
            f.sql(&format!("DROP TRIGGER text_http_fault ON {}.collaboration_artifact_outbox",f.artifacts)).await;
            let retained=p.files();
            error(&route,request(&p,0,"POST",PATH,wire(2,"Unconfirmed text","Retain these unpublished immutable bytes")),StatusCode::CONFLICT,"conflict","unconfirmed").await;
            assert_eq!(before,snapshot(f).await);assert_eq!(retained,p.files());p.read_committed(&original,b"Preserve exact original").await;
            drop(route);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
