use super::{
    child::{safe_threads, stopped},
    fixture::*,
    native_fixture::{self as http, Hosted},
};
use cyanrex_engine::{
    models::collaboration::ArtifactRevision,
    services::task_store::{TaskContentSnapshot, TaskContentStore},
};
use reqwest::Method;
use serde_json::{json, Value};

async fn unchanged_task(p: &Prepared<'_>, old: &TaskContentSnapshot) {
    super::schema::path(p.f, &p.f.task).await;
    let saved = TaskContentStore::new(p.f.owner().clone(), p.f.config().scope())
        .get(old.task.reference, old.task.owner)
        .await
        .unwrap();
    super::schema::path(p.f, &p.f.source).await;
    assert_eq!(saved, Some(old.clone()));
}

fn published(
    p: &Prepared<'_>,
    reply: Value,
    old: &TaskContentSnapshot,
    n: u128,
    text: &str,
) -> ArtifactRevision {
    assert_eq!(reply.as_object().unwrap().len(), 2);
    assert_eq!(reply["schema_version"], 1);
    for field in ["text", "task", "manifest", "contents"] {
        assert!(reply.get(field).is_none());
    }
    let artifact: ArtifactRevision = serde_json::from_value(reply["artifact"].clone()).unwrap();
    assert_eq!(artifact.owner, old.task.owner);
    assert_eq!(artifact.title, old.task.title);
    assert_eq!(artifact.reference, p.reference(n, text.as_bytes()));
    assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
    assert_eq!(artifact.media_type, "text/plain");
    assert_eq!(artifact.byte_length, text.len() as u64);
    assert_eq!(u64::from(artifact.sequence), 1);
    assert!(artifact.parent.is_none());
    artifact
}

async fn finish_child(child: super::child::Running, host: &Hosted, p: &Prepared<'_>) {
    assert!(stopped(&child.stop().await));
    assert!(within(tokio::net::TcpStream::connect(host.bind))
        .await
        .is_err());
    assert!(super::storage_helpers::no_connections(p.f).await);
    assert!(host.unchanged());
}

async fn exercise(tls: bool) {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let old = p.task(10, 0, "Collaborative teaching document, not a code task").await;
        let host = Hosted::new(&p, tls).await;
        let initial_files = p.files();
        let mut child = host.spawn(false);
        assert!(child.ready().await, "default native host must report readiness");
        assert!(safe_threads(child.pid(), &host.identity));
        if tls {
            let before = snapshot(f).await; let files = p.files();
            assert!(http::unknown_ca_refused(&host).await, "empty trust must fail as UnknownIssuer, not timeout/reset");
            assert!(http::wrong_ip_refused(&host).await, "trusted CA with wrong literal IP must fail certificate validation");
            assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files());
        }
        let client = host.client();
        let token = http::login(&host, &client).await;
        http::committed_token(&p, &token).await;
        assert_eq!(http::call(&client, &host.origin, Method::GET, "/platform/v1/session", &token, None, 200).await,
            json!({"schema_version":1,"authenticated":true}));
        let before = snapshot(f).await; let files = p.files();
        assert_eq!(http::call(&client, &host.origin, Method::POST, &path(&old), &token,
            Some(payload(&old, 1, "Absent without the explicit flag")), 404).await,
            json!({"schema_version":1,"error":{"code":"not_found","outcome":"not_attempted"}}));
        assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files());
        let compatibility_text = "Existing unbound text publication remains compatible";
        let reply = http::call(&client, &host.origin, Method::POST, "/platform/v1/text-artifacts", &token,
            Some(json!({"schema_version":1,"artifact_id":artifact_id(90),"revision_id":revision_id(90),
                "title":old.task.title,"text":compatibility_text})), 201).await;
        let artifact = published(&p, reply, &old, 90, compatibility_text);
        p.read_committed(&artifact, compatibility_text.as_bytes()).await;
        unchanged_task(&p, &old).await;
        task_counts(f, 1, 1).await; p.counts(1).await;
        finish_child(child, &host, &p).await;

        let mut child = host.spawn(true);
        assert!(child.ready().await, "explicit Task-publication host must report readiness");
        assert!(safe_threads(child.pid(), &host.identity));
        // The naturally issued Session survives a clean host restart; it is not reissued/adopted.
        http::committed_token(&p, &token).await;
        assert_eq!(http::call(&client, &host.origin, Method::GET, "/platform/v1/session", &token, None, 200).await,
            json!({"schema_version":1,"authenticated":true}));
        let private = p.non_artifacts().await;
        let text = "협업 수업 노트😀\r\n".repeat(4096);
        let body = payload(&old, 1, &text);
        assert!(body.to_string().len() > 16 * 1024 && text.len() <= 256 * 1024);
        let reply = http::call(&client, &host.origin, Method::POST, &path(&old), &token, Some(body), 201).await;
        let artifact = published(&p, reply, &old, 1, &text);
        p.read_committed(&artifact, text.as_bytes()).await;
        assert_eq!(private, p.non_artifacts().await);
        unchanged_task(&p, &old).await;
        let reopened = http::call(&client, &host.origin, Method::GET,
            &format!("/platform/v1/tasks/{}", old.task.reference.task_id), &token, None, 200).await;
        assert_eq!(reopened["task"], json!(old.task));
        assert_eq!(reopened["manifest"], json!(old.manifest));
        assert_eq!(reopened["contents"], json!([]), "publication must not implicitly attach bytes");
        let before = snapshot(f).await; let files = p.files();
        assert_eq!(http::call(&client, &host.origin, Method::POST, &path(&old), &p.tokens[1],
            Some(payload(&old, 2, "Do not publish into another current actor")), 404).await,
            json!({"schema_version":1,"error":{"code":"not_found","outcome":"unconfirmed"}}));
        let mut stale = old.clone(); stale.task.revision = 2.try_into().unwrap();
        assert_eq!(http::call(&client, &host.origin, Method::POST, &path(&stale), &token,
            Some(payload(&stale, 2, "Do not silently rebase")), 409).await,
            json!({"schema_version":1,"error":{"code":"stale_revision","outcome":"unconfirmed"}}));
        assert_eq!(http::call(&client, &host.origin, Method::GET, "/platform/", &token, None, 404).await,
            json!({"schema_version":1,"error":{"code":"not_found","outcome":"not_attempted"}}));
        assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files());
        let (headers, value) = http::capture(http::req(&client, &host.origin, Method::POST,
            "/platform/v1/session/logout", Some(&token)), 200).await;
        assert_eq!(value, json!({"schema_version":1,"ok":true}));
        assert_eq!(headers.get_all("set-cookie").iter().count(), 1);
        assert_eq!(headers["set-cookie"], if tls {
            "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure"
        } else { "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0" });
        let signed_out = snapshot(f).await;
        assert_eq!(http::call(&client, &host.origin, Method::POST, &path(&old), &token,
            Some(payload(&old, 2, "Revoked Session must not publish")), 401).await,
            json!({"schema_version":1,"error":{"code":"invalid_session","outcome":"unconfirmed"}}));
        assert_eq!(signed_out, snapshot(f).await); assert_eq!(files, p.files());
        assert!(safe_threads(child.pid(), &host.identity));
        finish_child(child, &host, &p).await;
        task_counts(f, 1, 1).await; p.counts(2).await;
        p.old_files_unchanged(&initial_files, 2);
        unchanged_task(&p, &old).await;
        drop((client, host)); assert!(p.finish());
    })).await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_task_text_publication_plain_flag_login_publish_and_shutdown() {
    exercise(false).await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL, OpenSSL and non-root capless child support"]
async fn postgres_platform_task_text_publication_tls_flag_private_ca_publish_and_shutdown() {
    exercise(true).await;
}
