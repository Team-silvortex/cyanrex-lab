use super::{run, Options};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

const COOKIE: &str = "cyanrex_session=native-smoke-session";

struct MockState {
    complete: bool,
    login: Mutex<Option<Value>>,
    submit: Mutex<Option<Value>>,
    cancel_count: AtomicUsize,
}

#[tokio::test]
async fn native_runner_smoke_authenticates_submits_and_accepts_a_real_result() {
    let (url, state, server) = mock_engine(true).await;
    let job_id = run(&options(url)).await.unwrap();
    assert_eq!(job_id, "job-native-smoke");
    assert_eq!(state.cancel_count.load(Ordering::SeqCst), 0);
    let login = state.login.lock().unwrap().clone().unwrap();
    assert_eq!(login["username"], "admin");
    assert_eq!(login["password"], "test-password");
    assert_eq!(login["otp"].as_str().unwrap().len(), 6);
    let submit = state.submit.lock().unwrap().clone().unwrap();
    assert_eq!(submit["agent_id"], "native-agent");
    assert_eq!(submit["program_name"], "agent-smoke");
    assert!(submit["code"]
        .as_str()
        .unwrap()
        .contains("cyanrex_agent_smoke"));
    server.abort();
}

#[tokio::test]
async fn native_runner_smoke_cancels_a_job_that_does_not_finish() {
    let (url, state, server) = mock_engine(false).await;
    let error = run(&options(url)).await.unwrap_err();
    assert!(error.contains("timed out"));
    assert_eq!(state.cancel_count.load(Ordering::SeqCst), 1);
    server.abort();
}

async fn mock_engine(complete: bool) -> (String, Arc<MockState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(MockState {
        complete,
        login: Mutex::new(None),
        submit: Mutex::new(None),
        cancel_count: AtomicUsize::new(0),
    });
    let app = Router::new()
        .route("/auth/login", post(login))
        .route("/ebpf/check/backends", get(backends))
        .route("/ebpf/check/remote", post(submit).get(status))
        .route("/ebpf/check/remote/cancel", post(cancel))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), state, server)
}

fn options(engine_url: String) -> Options {
    Options {
        engine_url,
        origin: "http://localhost:3000".to_owned(),
        agent_id: "native-agent".to_owned(),
        password: "test-password".to_owned(),
        totp_secret: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned(),
        ready_attempts: 1,
        ready_interval: Duration::ZERO,
        job_attempts: 1,
        job_interval: Duration::ZERO,
    }
}

async fn login(State(state): State<Arc<MockState>>, Json(body): Json<Value>) -> Response {
    *state.login.lock().unwrap() = Some(body);
    let mut response = Json(json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("cyanrex_session=native-smoke-session; HttpOnly; Path=/"),
    );
    response
}

async fn backends(headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"agents": [{"agent_id": "native-agent"}]})).into_response()
}

async fn submit(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authenticated(&headers) || headers.get(header::ORIGIN).is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    *state.submit.lock().unwrap() = Some(body);
    (
        StatusCode::ACCEPTED,
        Json(json!({"job_id": "job-native-smoke"})),
    )
        .into_response()
}

async fn status(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if !authenticated(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if state.complete {
        Json(json!({
            "state": "succeeded",
            "message": "completed",
            "result": {"ok": true}
        }))
        .into_response()
    } else {
        Json(json!({"state": "queued", "message": "queued", "result": null})).into_response()
    }
}

async fn cancel(State(state): State<Arc<MockState>>, headers: HeaderMap) -> Response {
    if !authenticated(&headers) || headers.get(header::ORIGIN).is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.cancel_count.fetch_add(1, Ordering::SeqCst);
    Json(json!({"state": "cancel_requested"})).into_response()
}

fn authenticated(headers: &HeaderMap) -> bool {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        == Some(COOKIE)
}
