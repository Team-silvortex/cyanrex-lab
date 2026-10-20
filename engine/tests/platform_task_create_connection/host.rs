use super::{
    child::{safe_threads, stopped, Running},
    expectation_helpers::Pins,
    fixture::*,
    kernel::{clear_child_capabilities, Identity},
    marker_helpers::MarkerPins,
};
use reqwest::{header::HeaderValue, Client, Method};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    net::TcpListener,
    os::unix::fs::OpenOptionsExt,
    process::{Command, Stdio},
    time::Duration,
};

async fn call(
    client: &Client,
    origin: &str,
    method: Method,
    path: &str,
    token: &str,
    body: Option<Value>,
    status: u16,
) -> Value {
    let mut cookie = HeaderValue::from_str(&format!("cyanrex_platform_session={token}")).unwrap();
    cookie.set_sensitive(true);
    let mut request = client
        .request(method, format!("{origin}{path}"))
        .header("origin", origin)
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", cookie);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("owned Task creation HTTP request failed"));
    assert_eq!(response.status().as_u16(), status);
    for (key, expected) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(response.headers()[key], expected);
    }
    for key in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
    ] {
        assert!(!response.headers().contains_key(key));
    }
    if status == 405 {
        assert_eq!(
            response.headers()["allow"],
            if path == "/platform/v1/tasks" {
                "POST"
            } else if path.ends_with("/content") {
                "PUT"
            } else {
                "GET"
            }
        );
    }
    let bytes = response
        .bytes()
        .await
        .unwrap_or_else(|_| panic!("owned Task creation HTTP body failed"));
    assert!(bytes.len() <= 64 * 1024);
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_task_create_real_loopback_host_creates_lists_and_reopens_private_empty_work(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;
        p.enable_reopen().await;
        let identity=Identity::current();assert!(identity.safe_ids());
        let reserve=TcpListener::bind(("127.0.0.1",0)).unwrap();let bind=reserve.local_addr().unwrap();drop(reserve);
        let origin=format!("http://{bind}");let c=f.config();let options=f.runtime().connect_options();
        let transport=match options.get_socket(){Some(path)=>json!({"kind":"unix","directory":path}),None=>json!({"kind":"loopback","address":options.get_host()})};
        let document=json!({"format_version":3,"origin":origin,
            "authority_id":c.scope().authority_id.as_uuid(),"workspace_id":c.scope().workspace_id.as_uuid(),
            "namespaces":{"source":f.source,"task":f.task,"artifacts":f.artifacts},
            "database":{"database":f.database,"role":f.role,"system_identifier":c.target().system_identifier(),"database_oid":c.target().database_oid(),"transport":transport,"port":options.get_port(),"password":super::storage_helpers::PASSWORD},
            "session_limits":{"per_account":c.limits().per_account(),"total":c.limits().total()},"artifact_root":p.root.path(),"catalog_identity":Pins::read(f).await.json(),"marker_identity":MarkerPins::read(f).await.json()});
        let original=serde_json::to_vec(&document).unwrap();let path=p.root.path().parent().unwrap().join("create-host-input.json");
        let mut file=OpenOptions::new().create_new(true).write(true).mode(0o600).open(&path).unwrap();file.write_all(&original).unwrap();drop(file);
        let groups=if identity.groups.is_empty(){"none".to_owned()}else{identity.groups.iter().map(u32::to_string).collect::<Vec<_>>().join(",")};
        let mut command=Command::new(env!("CARGO_BIN_EXE_cyanrex-platform"));
        command.args(["serve-loopback","--uid",&identity.uid[0].to_string(),"--gid",&identity.gid[0].to_string(),"--groups",&groups,"--config"]).arg(&path).args(["--bind",&bind.to_string()]).current_dir(path.parent().unwrap()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        for(name,_)in std::env::vars_os(){if name.as_encoded_bytes().starts_with(b"PG"){command.env_remove(name);}}
        command.env_remove("DATABASE_URL").env_remove("NOTIFY_SOCKET");clear_child_capabilities(&mut command);
        let mut child=Running::spawn(&mut command);let ready=child.ready().await;let mut valid=false;
        if ready {
            assert!(safe_threads(child.pid(),&identity));
            let client=Client::builder().http1_only().no_proxy().redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(2)).timeout(Duration::from_secs(10)).build().unwrap();
            let created=call(&client,&origin,Method::POST,"/platform/v1/tasks",&p.tokens[0],Some(json!({"schema_version":1,"task_id":task_id(10),"title":"备课可以没有代码和 payload😀"})),201).await;
            assert_eq!(created["task"]["owner"],json!(p.owners[0]));assert_eq!(created["manifest"]["payload"],json!([]));assert!(created.get("contents").is_none());
            let page=call(&client,&origin,Method::GET,"/platform/v1/task-index",&p.tokens[0],None,200).await;
            assert_eq!(page["entries"].as_array().unwrap().len(),1);assert_eq!(page["entries"][0]["payload_items"],0);
            let task_path=format!("/platform/v1/tasks/{}",task_id(10));
            let reopened=call(&client,&origin,Method::GET,&task_path,&p.tokens[0],None,200).await;
            assert_eq!(reopened["task"],created["task"]);assert_eq!(reopened["manifest"],created["manifest"]);assert_eq!(reopened["contents"],json!([]));
            assert_eq!(call(&client,&origin,Method::GET,&task_path,&p.tokens[1],None,404).await,json!({"schema_version":1,"error":{"code":"not_found","outcome":"unconfirmed"}}));
            assert_eq!(call(&client,&origin,Method::GET,"/platform/v1/tasks",&p.tokens[0],None,405).await,json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}}));
            assert_eq!(call(&client,&origin,Method::POST,&task_path,&p.tokens[0],None,405).await,json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}}));
            assert_eq!(call(&client,&origin,Method::GET,&format!("{task_path}/content"),&p.tokens[0],None,405).await,json!({"schema_version":1,"error":{"code":"method_not_allowed","outcome":"not_attempted"}}));
            valid=true;
        }
        let run=child.stop().await;assert!(ready&&valid&&stopped(&run),"real host must complete explicit empty Task creation workflow");
        assert!(tokio::time::timeout(Duration::from_secs(1),tokio::net::TcpStream::connect(bind)).await.is_ok_and(|r|r.is_err()));
        assert!(super::storage_helpers::no_connections(f).await);assert_eq!(p.count("collaboration_tasks").await,1);assert_eq!(p.count("collaboration_task_outbox").await,1);
        assert_eq!(std::fs::read_dir(p.root.path()).unwrap().count(),1);assert!(fs::read(&path).is_ok_and(|bytes|bytes==original));assert!(identity==Identity::current());assert!(p.finish());
    })).await;
}
