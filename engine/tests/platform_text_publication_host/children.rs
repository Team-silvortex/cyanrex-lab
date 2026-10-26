use super::{
    child::{safe_threads, stopped},
    child_fixture::{self as http, Hosted},
    fixture::*,
};
use cyanrex_engine::models::collaboration::ArtifactRevision;
use reqwest::Method;
use serde_json::json;

async fn chain(p: &Prepared<'_>, host: &Hosted, client: &reqwest::Client, token: &str) {
    let files = p.files();
    let text = "课文😀".repeat(4096);
    let body = json!({"schema_version":1,"artifact_id":artifact_id(1),"revision_id":revision_id(1),"title":"Teaching material, not program source","text":text});
    assert!(
        body.to_string().len() > 16 * 1024 && text.len() <= 256 * 1024,
        "real HTTP body must cross the ingress header-buffer size"
    );
    let before = p.non_artifacts().await;
    let created = http::call(
        client,
        &host.origin,
        Method::POST,
        PATH,
        token,
        Some(body),
        201,
    )
    .await;
    assert_eq!(created.as_object().unwrap().len(), 2);
    let artifact: ArtifactRevision = serde_json::from_value(created["artifact"].clone()).unwrap();
    assert_eq!(artifact.owner, p.owners[0]);
    assert_eq!(artifact.reference, p.reference(1, text.as_bytes()));
    assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
    assert_eq!(artifact.media_type, "text/plain");
    assert_eq!(artifact.byte_length, text.len() as u64);
    assert_eq!(before, p.non_artifacts().await);
    p.read_committed(&artifact, text.as_bytes()).await;
    let id = task_id(1);
    let task_path = format!("/platform/v1/tasks/{id}");
    let edit_path = format!("{task_path}/content");
    let empty = http::call(
        client,
        &host.origin,
        Method::POST,
        "/platform/v1/tasks",
        token,
        Some(json!({"schema_version":1,"task_id":id,"title":"A teaching collaboration task"})),
        201,
    )
    .await;
    assert_eq!(empty["manifest"]["payload"], json!([]));
    assert_eq!(empty["task"]["revision"], 1);
    assert!(empty["task"]["definition_ref"].is_null());
    let requested = manifest(
        "Text is one optional payload",
        vec![artifact.reference.clone()],
    );
    let changed = http::call(
        client,
        &host.origin,
        Method::PUT,
        &edit_path,
        token,
        Some(json!({"schema_version":1,"expected_revision":1,"manifest":requested})),
        200,
    )
    .await;
    assert_eq!(changed["task"]["revision"], 2);
    assert_eq!(changed["task"]["owner"], json!(p.owners[0]));
    assert_eq!(changed["manifest"], json!(requested));
    assert!(changed.get("contents").is_none());
    let reopened = http::call(
        client,
        &host.origin,
        Method::GET,
        &task_path,
        token,
        None,
        200,
    )
    .await;
    assert_eq!(reopened["task"], changed["task"]);
    assert_eq!(reopened["manifest"], changed["manifest"]);
    assert_eq!(
        reopened["contents"],
        json!([{"artifact":artifact.reference,"text":text}])
    );
    let page = http::call(
        client,
        &host.origin,
        Method::GET,
        "/platform/v1/task-index",
        token,
        None,
        200,
    )
    .await;
    assert_eq!(page["entries"][0]["title"], requested.title());
    assert_eq!(page["entries"][0]["payload_items"], 1);
    let foreign = http::call(
        client,
        &host.origin,
        Method::GET,
        &task_path,
        &p.tokens[1],
        None,
        404,
    )
    .await;
    assert_eq!(
        foreign,
        json!({"schema_version":1,"error":{"code":"not_found","outcome":"unconfirmed"}})
    );
    let foreign_index = http::call(
        client,
        &host.origin,
        Method::GET,
        "/platform/v1/task-index",
        &p.tokens[1],
        None,
        200,
    )
    .await;
    assert_eq!(foreign_index["entries"], json!([]));
    let unmounted = http::call(
        client,
        &host.origin,
        Method::POST,
        &format!("{task_path}/status"),
        token,
        None,
        404,
    )
    .await;
    assert_eq!(
        unmounted,
        json!({"schema_version":1,"error":{"code":"not_found","outcome":"not_attempted"}})
    );
    let duplicate=http::call(client,&host.origin,Method::POST,PATH,token,Some(json!({"schema_version":1,"artifact_id":artifact_id(1),"revision_id":revision_id(1),"title":"Do not replay","text":"A different new request body"})),409).await;
    assert_eq!(
        duplicate,
        json!({"schema_version":1,"error":{"code":"conflict","outcome":"unconfirmed"}})
    );
    p.counts(1).await;
    task_counts(p.f, 1, 2).await;
    p.old_files_unchanged(&files, 1);
}

async fn exercise(tls: bool) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let host=Hosted::new(&p,tls).await;let mut child=host.spawn();
        let ready=child.ready().await;assert!(ready,"real publication child never reported readiness");assert!(safe_threads(child.pid(),&host.identity));
        if tls {
            let before=snapshot(f).await;let files=p.files();
            assert!(http::unknown_ca_refused(&host).await,"an empty CA store must reject the correct literal IP with certificate UnknownIssuer, without sending credentials");
            assert!(http::wrong_ip_refused(&host).await,"trusted CA with wrong literal IP must fail client certificate validation");
            assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());
        }
        let client=host.client(true);let token=http::login(&host,&client).await;http::committed_token(&p,&token).await;
        assert_eq!(http::call(&client,&host.origin,Method::GET,"/platform/v1/session",&token,None,200).await,json!({"schema_version":1,"authenticated":true}));
        chain(&p,&host,&client,&token).await;
        let(headers,value)=http::capture(http::req(&client,&host.origin,Method::POST,"/platform/v1/session/logout",Some(&token)),200).await;
        assert_eq!(value,json!({"schema_version":1,"ok":true}));assert_eq!(headers.get_all("set-cookie").iter().count(),1);
        assert_eq!(headers["set-cookie"],if tls{"cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure"}else{"cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0"});
        assert_eq!(http::call(&client,&host.origin,Method::POST,PATH,&token,Some(json!({"schema_version":1,"artifact_id":artifact_id(2),"revision_id":revision_id(2),"title":"Revoked new publication","text":"Must never be published"})),401).await,json!({"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}));
        let result=child.stop().await;assert!(stopped(&result));assert!(within(tokio::net::TcpStream::connect(host.bind)).await.is_err());
        assert!(super::storage_helpers::no_connections(f).await);assert!(host.unchanged());p.counts(1).await;task_counts(f,1,2).await;
        drop((client,host));assert!(p.finish());
    })).await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_text_publication_plain_child_login_publish_attach_reopen_and_shutdown() {
    exercise(false).await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_text_publication_tls_child_ca_ip_san_cookie_publish_and_shutdown() {
    exercise(true).await;
}
