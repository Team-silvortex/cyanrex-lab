//! Synthetic transport-only results do not establish successful persistence or authorization.
use super::{
    build_task_content_replace_router, Backend as ReplaceBackend, TaskContentReplaceHttpState,
};
use crate::{
    models::collaboration::{
        CoreSchemaVersion, PrincipalRef, RevisionNumber, TaskContentManifest, TaskId, TaskRef,
        TaskSnapshot, TaskStatus, WorkspaceRef,
    },
    services::{
        auth_service::durable_source::{SessionCommandError, SessionTaskError},
        collaboration_identity_store::IdentityStoreError,
        task_store::{TaskContentSnapshot, TaskStoreError},
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
use tokio::sync::{Notify, Semaphore};
use tower::ServiceExt;
use uuid::Uuid;

pub(in crate::platform_http) const ORIGIN: &str = "https://platform.example";
pub(in crate::platform_http) const REPLACE: &str =
    "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content";
pub(super) const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub(super) const ID: &str = "e3698554-8eb3-4b6d-884e-8074e1555136";

#[derive(Clone, Copy)]
pub(in crate::platform_http) enum Outcome {
    Success,
    Unavailable,
    InvalidSession,
    Denied,
    Missing,
    Stale,
    NonDraft,
    Noop,
}

pub(super) struct Received {
    pub(super) token: String,
    pub(super) id: TaskId,
    pub(super) revision: RevisionNumber,
    pub(super) manifest: TaskContentManifest,
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
    pub(super) fn blocking(outcome: Outcome) -> Arc<Self> {
        Self::configured(outcome, true)
    }
    fn configured(outcome: Outcome, blocked: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            received: Mutex::new(vec![]),
            outcome,
            blocked,
            started: Semaphore::new(0),
            released: Semaphore::new(0),
        })
    }
    pub(in crate::platform_http) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub(super) fn received(&self) -> Vec<Received> {
        std::mem::take(&mut *self.received.lock().unwrap())
    }
    pub(super) fn release(&self) {
        self.released.add_permits(1);
    }
    pub(super) async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(2), self.started.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
    }
    pub(super) async fn replace(
        &self,
        token: &str,
        id: TaskId,
        revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.received.lock().unwrap().push(Received {
            token: token.to_owned(),
            id,
            revision,
            manifest: manifest.clone(),
        });
        self.started.add_permits(1);
        if self.blocked {
            self.released.acquire().await.unwrap().forget();
        }
        match self.outcome {
            Outcome::Unavailable => Err(TaskStoreError::StorageUnavailable.into()),
            Outcome::InvalidSession => Err(SessionCommandError::InvalidSession.into()),
            Outcome::Denied => {
                Err(SessionCommandError::Identity(IdentityStoreError::AccessDenied).into())
            }
            Outcome::Missing => Err(TaskStoreError::NotFound.into()),
            Outcome::Stale => Err(TaskStoreError::StaleRevision.into()),
            Outcome::NonDraft => Err(TaskStoreError::InvalidTransition.into()),
            Outcome::Noop => Err(TaskStoreError::InvalidInput.into()),
            Outcome::Success => {
                let workspace = WorkspaceRef {
                    authority_id: Uuid::from_u128(4201).try_into().unwrap(),
                    workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
                };
                let now = Utc.timestamp_opt(1_800_000_000, 0).single().unwrap();
                Ok(TaskContentSnapshot {
                    task: TaskSnapshot {
                        schema_version: CoreSchemaVersion,
                        reference: TaskRef {
                            workspace,
                            task_id: id,
                        },
                        owner: PrincipalRef {
                            authority_id: workspace.authority_id,
                            principal_id: Uuid::from_u128(4203).try_into().unwrap(),
                        },
                        title: manifest.title().to_owned(),
                        definition: None,
                        input_refs: manifest.input_refs(),
                        status: TaskStatus::Draft,
                        revision: RevisionNumber::try_from(u64::from(revision) + 1).unwrap(),
                        created_at: now,
                        updated_at: now,
                    },
                    manifest,
                })
            }
        }
    }
}

pub(in crate::platform_http) fn state(
    backend: Arc<Backend>,
    origin: &str,
    capacity: usize,
) -> TaskContentReplaceHttpState {
    TaskContentReplaceHttpState::configured(ReplaceBackend::Test(backend), origin, capacity)
        .unwrap()
}
pub(super) fn app(backend: Arc<Backend>, capacity: usize) -> Router {
    build_task_content_replace_router(state(backend, ORIGIN, capacity))
}
pub(in crate::platform_http) fn body() -> Body {
    Body::from(wire().to_string())
}
pub(super) fn wire() -> Value {
    json!({"schema_version":1,"expected_revision":1,"manifest":{"schema_version":1,"title":"Renamed without code","payload":[]}})
}
pub(super) fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("cyanrex_platform_session={TOKEN}"))
        .body(body)
        .unwrap();
    request
        .headers_mut()
        .get_mut(header::COOKIE)
        .unwrap()
        .set_sensitive(true);
    request
}
pub(super) fn untouched() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("rejected replace must not poll body")
        },
    ))
}
pub(super) fn pending_body() -> (Body, Arc<Notify>) {
    let entered = Arc::new(Notify::new());
    let observed = entered.clone();
    (
        Body::from_stream(futures_util::stream::poll_fn(move |_| {
            observed.notify_one();
            std::task::Poll::<Option<Result<Bytes, std::io::Error>>>::Pending
        })),
        entered,
    )
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
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "access-control-allow-credentials",
    ] {
        assert!(!response.headers.contains_key(name));
    }
    assert!(!response.json.to_string().contains(TOKEN));
}
pub(super) fn error(response: &Captured, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private(response);
    assert_eq!(
        response.json,
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
