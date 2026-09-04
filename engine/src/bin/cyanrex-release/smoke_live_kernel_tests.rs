use super::{run, Options};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;

const COOKIE: &str = "cyanrex_session=native-kernel-session";
const PIN_PATH: &str = "/sys/fs/bpf/cyanrex/native-kernel-smoke";

struct MockState {
    emit_event: bool,
    prove_exact_detach: bool,
    attached: AtomicBool,
    detach_count: AtomicUsize,
    program_name: Mutex<Option<String>>,
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyanrex-live-kernel-smoke-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn native_live_kernel_smoke_writes_evidence_after_exact_cleanup() {
    let fixture = Fixture::new();
    let report = fixture.root.join("evidence.json");
    let (url, state, server) = mock_engine(true, true).await;
    let outcome = run(&options(url, Some(report.clone()))).await.unwrap();
    assert_eq!(outcome.pin_path, PIN_PATH);
    assert_eq!(outcome.report.as_deref(), Some(report.as_path()));
    assert!(!state.attached.load(Ordering::SeqCst));
    assert_eq!(state.detach_count.load(Ordering::SeqCst), 1);
    let evidence: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(evidence["schemaVersion"], 2);
    assert_eq!(evidence["result"], "passed");
    assert!(evidence["candidate"].is_null());
    assert_eq!(
        evidence["exercise"]["event"]["programName"],
        outcome.program_name
    );
    assert_eq!(evidence["cleanup"]["exactPinDetached"], true);
    server.abort();
}

#[tokio::test]
async fn native_live_kernel_smoke_detaches_when_no_matching_event_arrives() {
    let (url, state, server) = mock_engine(false, true).await;
    let error = run(&options(url, None)).await.unwrap_err();
    assert!(error.contains("no live kernel ringbuf event"));
    assert!(!state.attached.load(Ordering::SeqCst));
    assert_eq!(state.detach_count.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn native_live_kernel_smoke_reports_unproven_failure_cleanup() {
    let (url, state, server) = mock_engine(false, false).await;
    let error = run(&options(url, None)).await.unwrap_err();
    assert!(error.contains("no live kernel ringbuf event"));
    assert!(error.contains("additionally failed to prove clean detach"));
    assert_eq!(state.detach_count.load(Ordering::SeqCst), 1);
    server.abort();
}

async fn mock_engine(
    emit_event: bool,
    prove_exact_detach: bool,
) -> (String, Arc<MockState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(MockState {
        emit_event,
        prove_exact_detach,
        attached: AtomicBool::new(false),
        detach_count: AtomicUsize::new(0),
        program_name: Mutex::new(None),
    });
    let app = Router::new()
        .route("/auth/login", post(login))
        .route("/helper/environment", get(environment))
        .route("/ebpf/attachments", get(attachments))
        .route("/ebpf/templates", get(templates))
        .route("/ebpf/run", post(attach))
        .route("/events", get(events))
        .route("/ebpf/detach", post(detach))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), state, server)
}

fn options(engine_url: String, report: Option<PathBuf>) -> Options {
    Options {
        engine_url,
        origin: "http://localhost:3000".to_owned(),
        password: "test-password".to_owned(),
        totp_secret: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        stream_seconds: 2,
        poll_attempts: 1,
        poll_interval: Duration::ZERO,
        report,
        release_metadata: None,
    }
}

async fn login() -> Response {
    let mut response = Json(json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("cyanrex_session=native-kernel-session; HttpOnly; Path=/"),
    );
    response
}

async fn environment(headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({
        "overall_ok": true,
        "generated_at": "2026-09-05T01:02:03Z",
        "runtime_mode": "docker",
        "checks": [{"name": "kernel", "ok": true, "detail": "mock 6.8.0 kernel"}]
    }))
    .into_response()
}

async fn templates(headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!([{"id": "ringbuf-hi-freq-sampler", "code": "int main(void) { return 0; }"}]))
        .into_response()
}

async fn attachments(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let paths = if state.attached.load(Ordering::SeqCst) {
        vec![PIN_PATH]
    } else {
        Vec::new()
    };
    Json(json!({"pin_paths": paths})).into_response()
}

async fn attach(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authenticated_origin(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    *state.program_name.lock().unwrap() = body["program_name"].as_str().map(str::to_owned);
    state.attached.store(true, Ordering::SeqCst);
    Json(json!({
        "success": true,
        "stage": "run",
        "message": "attached",
        "pin_path": PIN_PATH
    }))
    .into_response()
}

async fn events(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let program = state.program_name.lock().unwrap().clone().unwrap();
    let events = if state.emit_event {
        vec![json!({
            "timestamp": "2026-09-05T01:02:04Z",
            "event_type": "ebpf.kernel_ringbuf",
            "payload": {"program_name": program, "bytes": 64}
        })]
    } else {
        Vec::new()
    };
    Json(events).into_response()
}

async fn detach(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if !authenticated_origin(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.attached.store(false, Ordering::SeqCst);
    state.detach_count.fetch_add(1, Ordering::SeqCst);
    let detached = if state.prove_exact_detach {
        vec![PIN_PATH]
    } else {
        Vec::new()
    };
    Json(json!({"ok": true, "clean": true, "detached": detached})).into_response()
}

fn authenticated(headers: &HeaderMap) -> bool {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        == Some(COOKIE)
}

fn authenticated_origin(headers: &HeaderMap) -> bool {
    authenticated(headers) && headers.get(header::ORIGIN).is_some()
}
