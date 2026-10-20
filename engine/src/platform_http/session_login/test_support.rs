use super::{build_session_login_router, Backend as LoginBackend, SessionLoginHttpState};
use crate::{
    models::collaboration::LegacyUsername,
    services::auth_service::durable_source::{
        DurableAccountRef, DurableAuthError, DurableLogin, DurableSession,
    },
};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, Request, StatusCode},
    response::Response,
    Router,
};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;
use uuid::Uuid;

pub(super) const ORIGIN: &str = "https://platform.example";
pub(super) const LOGIN: &str = "/platform/v1/session/login";
pub(super) const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub(super) const PASSWORD: &str = "public-synthetic-password-only";
pub(super) const OTP: &str = "123456";

#[derive(Clone, Copy)]
pub(in crate::platform_http) enum Outcome {
    Success,
    Error(DurableAuthError),
    Token(&'static str),
}

// These synthetic input records intentionally have no Debug/Serialize implementation.
pub(super) struct Received {
    pub(super) username: String,
    pub(super) password: String,
    pub(super) otp: String,
}

pub(in crate::platform_http) struct Backend {
    calls: AtomicUsize,
    received: Mutex<Vec<Received>>,
    outcome: Outcome,
    blocked: bool,
    started: Semaphore,
    released: Semaphore,
}

impl Backend {
    pub(in crate::platform_http) fn new(outcome: Outcome) -> Arc<Self> {
        Self::configured(outcome, false)
    }
    pub(in crate::platform_http) fn blocking(outcome: Outcome) -> Arc<Self> {
        Self::configured(outcome, true)
    }
    fn configured(outcome: Outcome, blocked: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            received: Mutex::new(Vec::new()),
            outcome,
            blocked,
            started: Semaphore::new(0),
            released: Semaphore::new(0),
        })
    }
    pub(in crate::platform_http) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub(super) fn take_received(&self) -> Vec<Received> {
        std::mem::take(&mut *self.received.lock().unwrap())
    }
    pub(in crate::platform_http) fn release(&self, count: usize) {
        self.released.add_permits(count);
    }
    pub(in crate::platform_http) async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(2), self.started.acquire())
            .await
            .expect("synthetic backend was not dispatched")
            .unwrap()
            .forget();
    }
    pub(super) async fn login(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<DurableLogin, DurableAuthError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.received.lock().unwrap().push(Received {
            username: username.to_string(),
            password: password.to_owned(),
            otp: otp.to_owned(),
        });
        self.started.add_permits(1);
        if self.blocked {
            self.released.acquire().await.unwrap().forget();
        }
        let token = match self.outcome {
            Outcome::Error(error) => return Err(error),
            Outcome::Success => TOKEN,
            Outcome::Token(value) => value,
        };
        Ok(DurableLogin {
            token: token.to_owned(),
            session: DurableSession {
                account: DurableAccountRef {
                    authority_id: Uuid::from_u128(4201).try_into().unwrap(),
                    username: username.clone(),
                    account_id: Uuid::from_u128(8211).try_into().unwrap(),
                },
                expires_at: Utc.timestamp_opt(4_102_444_800, 0).single().unwrap(),
            },
        })
    }
}

pub(in crate::platform_http) fn state(
    backend: Arc<Backend>,
    origin: &str,
    capacity: usize,
) -> SessionLoginHttpState {
    SessionLoginHttpState::configured(LoginBackend::Test(backend), origin, capacity).unwrap()
}
pub(super) fn app(backend: Arc<Backend>) -> Router {
    build_session_login_router(state(backend, ORIGIN, 4))
}
pub(super) fn valid_json() -> Value {
    json!({"schema_version":1,"username":"synthetic-user","password":PASSWORD,"otp":OTP})
}
pub(super) fn valid_body() -> Body {
    Body::from(serde_json::to_vec(&valid_json()).unwrap())
}
pub(super) fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap()
}
pub(super) fn untouched() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("pre-dispatch rejection must not poll the body")
        },
    ))
}

pub(super) struct Captured {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) json: Value,
}
pub(super) async fn capture(app: Router, request: Request<Body>) -> Captured {
    capture_response(app.oneshot(request).await.unwrap()).await
}
pub(super) async fn capture_response(response: Response) -> Captured {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    Captured {
        status,
        headers,
        json,
    }
}
pub(super) fn private(response: &Captured) {
    assert_eq!(response.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers[header::PRAGMA], "no-cache");
    assert_eq!(response.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    for name in [
        "location",
        "retry-after",
        "access-control-allow-origin",
        "access-control-allow-credentials",
    ] {
        assert!(
            !response.headers.contains_key(name),
            "private login response exposed an extra transport header"
        );
    }
    let text = response.json.to_string();
    for secret in [TOKEN, PASSWORD, OTP] {
        assert!(
            !text.contains(secret),
            "login response exposed synthetic secret input"
        );
    }
}
pub(super) fn error(response: &Captured, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private(response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(response.headers[header::CONTENT_TYPE], "application/json");
    assert!(
        response.json == json!({"schema_version":1,"error":{"code":code,"outcome":outcome}}),
        "private error projection diverged"
    );
}
pub(super) fn success(response: &Captured, secure: bool) {
    assert_eq!(response.status, StatusCode::OK);
    private(response);
    assert!(
        response.json == json!({"schema_version":1,"authenticated":true}),
        "success must expose only the minimal committed projection"
    );
    assert_eq!(
        response.headers.get_all(header::SET_COOKIE).iter().count(),
        1
    );
    let cookie = response.headers.get(header::SET_COOKIE).unwrap();
    assert!(
        cookie.is_sensitive(),
        "session Set-Cookie must be sensitive"
    );
    let expected = format!(
        "cyanrex_platform_session={TOKEN}; Path=/platform/v1; HttpOnly; SameSite=Strict{}",
        if secure { "; Secure" } else { "" }
    );
    assert!(
        cookie.as_bytes() == expected.as_bytes(),
        "issuance must match the fixed host-only browser-session cookie policy"
    );
}
