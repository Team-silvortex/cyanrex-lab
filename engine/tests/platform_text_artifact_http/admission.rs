use super::fixture::*;
use axum::{
    body::{Body, Bytes},
    http::{HeaderValue, StatusCode},
};
use cyanrex_engine::models::collaboration::MAX_TEXT_PAYLOAD_BYTES;
use serde_json::json;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_http_bad_admission_and_strict_body_never_write() {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await; let route=router(&p,1).await;
        let before=snapshot(f).await; let files=p.files();
        for method in ["GET","PUT","PATCH","DELETE","OPTIONS"] {
            error(&route,request(&p,0,method,PATH,unread()),StatusCode::METHOD_NOT_ALLOWED,"method_not_allowed","not_attempted").await;
        }
        for path in ["/platform/v1/text-artifacts/","/platform/v1/text-artifacts/extra","/platform/v1/tasks","/platform/v1/artifacts"] {
            error(&route,request(&p,0,"POST",path,unread()),StatusCode::NOT_FOUND,"not_found","not_attempted").await;
        }
        error(&route,request(&p,0,"POST","/platform/v1/text-artifacts?owner=other",unread()),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;
        for key in ["origin","x-cyanrex-platform-request"] {
            for mode in ["missing","duplicate","wrong"] {
                let mut req=request(&p,0,"POST",PATH,unread());
                if mode=="missing" {req.headers_mut().remove(key);}
                else if mode=="duplicate" {let value=req.headers()[key].clone();req.headers_mut().append(key,value);}
                else {req.headers_mut().insert(key,HeaderValue::from_static("wrong"));}
                error(&route,req,StatusCode::FORBIDDEN,"request_forbidden","not_attempted").await;
            }
        }
        for cookie in [String::new(),"session=legacy".into(),"cyanrex_platform_session=bad".into(),format!("cyanrex_platform_session={0}; cyanrex_platform_session={0}",p.tokens[0])] {
            let mut req=request(&p,0,"POST",PATH,unread()); req.headers_mut().insert("cookie",HeaderValue::from_str(&cookie).unwrap());
            error(&route,req,StatusCode::UNAUTHORIZED,"invalid_session","not_attempted").await;
        }
        let mut req=request(&p,0,"POST",PATH,unread());req.headers_mut().insert("authorization",HeaderValue::from_static("Bearer not-a-session"));
        error(&route,req,StatusCode::UNAUTHORIZED,"invalid_session","not_attempted").await;
        for (key,value,status,code) in [("content-type","text/plain",StatusCode::UNSUPPORTED_MEDIA_TYPE,"unsupported_media_type"),("content-encoding","gzip",StatusCode::BAD_REQUEST,"invalid_request")] {
            let mut req=request(&p,0,"POST",PATH,unread());req.headers_mut().insert(key,HeaderValue::from_static(value));
            error(&route,req,status,code,"not_attempted").await;
        }
        let valid=json!({"schema_version":1,"artifact_id":artifact_id(20),"revision_id":revision_id(20),"title":"Explicit title","text":"valid"});
        let raw=valid.to_string();
        let mut invalid=vec!["null".into(),"[]".into(),"{}".into(),format!("\u{feff}{raw}"),format!("{raw} {{}}")];
        for key in ["schema_version","artifact_id","revision_id","title","text"] {
            invalid.push(raw.replacen(&format!("\"{key}\":"),&format!("\"{key}\":null,\"{key}\":"),1));
        }
        for key in ["owner","scope","namespace","path","sha256","kind","media_type","filename","expected_revision"] {
            let mut value=valid.clone();value[key]=json!("not caller authority");invalid.push(value.to_string());
        }
        for (key,bad) in [("schema_version",json!(2)),("schema_version",json!(1.0)),("artifact_id",json!(uuid::Uuid::nil())),("revision_id",json!("not-an-id")),("title",json!("   ")),("title",json!("x".repeat(257))),("title",json!("bad\nline")),("text",json!("x".repeat(MAX_TEXT_PAYLOAD_BYTES+1))),("text",json!("bad\u{0085}control"))] {
            let mut value=valid.clone();value[key]=bad;invalid.push(value.to_string());
        }
        for body in invalid {error(&route,request(&p,0,"POST",PATH,Body::from(body)),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;}
        error(&route,request(&p,0,"POST",PATH,Body::from(vec![0xff,0xfe])),StatusCode::BAD_REQUEST,"invalid_request","not_attempted").await;
        // Actual streamed bytes, not a misleading Content-Length, enforce the wire bound.
        let exact=format!("{raw}{}"," ".repeat(2*1024*1024-raw.len()));
        let stream=futures_util::stream::iter([Ok::<_,std::io::Error>(Bytes::from(exact)),Ok(Bytes::from_static(b" "))]);
        let mut req=request(&p,0,"POST",PATH,Body::from_stream(stream));req.headers_mut().insert("content-length",HeaderValue::from_static("1"));
        error(&route,req,StatusCode::PAYLOAD_TOO_LARGE,"request_too_large","not_attempted").await;
        let pending=futures_util::stream::pending::<Result<Bytes,std::io::Error>>();
        error(&route,request(&p,0,"POST",PATH,Body::from_stream(pending)),StatusCode::REQUEST_TIMEOUT,"request_timeout","not_attempted").await;
        assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());p.counts(0).await;
        drop(route);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
