use crate::{fixture::*, *};
use axum::response::Response;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Notify, Semaphore};

pub fn auth(f: &Fixture) -> &identity_fixture::Fixture {
    &f.sharing.base.grants.auth
}
pub fn bounded(f: &Fixture) -> Router {
    build_file_download_manifest_router(
        FileDownloadManifestHttpState::with_capacity(
            auth(f).source.clone(),
            f.workspace.clone(),
            ORIGIN,
            1,
        )
        .unwrap(),
    )
}
pub fn body_request(token: &str, path: &str, body: Body) -> Request<Body> {
    let mut request = request(token, path);
    *request.body_mut() = body;
    request
}
pub fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("cheap rejection must not poll actual request bytes")
        },
    ))
}
pub fn headers(response: &Response) {
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["pragma"], "no-cache");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    for name in [
        "set-cookie",
        "location",
        "retry-after",
        "access-control-allow-origin",
        "content-disposition",
        "content-range",
        "etag",
        "last-modified",
        "x-cyanrex-owner-id",
        "x-cyanrex-account-id",
        "x-cyanrex-source-key",
        "x-cyanrex-root-key",
        "x-cyanrex-journal-key",
        "x-cyanrex-file-transfer-version",
        "x-cyanrex-file-id",
        "x-cyanrex-file-version-id",
        "x-cyanrex-file-sha256",
        "x-cyanrex-chunk-index",
        "x-cyanrex-chunk-sha256",
    ] {
        assert!(
            !response.headers().contains_key(name),
            "private metadata response must not expose {name}"
        );
    }
}
pub async fn error_response(response: Response, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status(), status);
    headers(&response);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
pub async fn error(
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    error_response(
        route.clone().oneshot(request).await.unwrap(),
        status,
        code,
        outcome,
    )
    .await;
}
pub async fn denied(
    f: &Fixture,
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
) {
    let before = f.sharing.snapshot().await;
    error(route, request, status, code, "unconfirmed").await;
    assert_eq!(f.sharing.snapshot().await, before);
}
pub async fn success(
    f: &Fixture,
    route: &Router,
    typed: &SessionFileDownloadRequest,
    expected: &FileTransferManifest,
) {
    let before = f.sharing.snapshot().await;
    let response = route
        .clone()
        .oneshot(request(&auth(f).target_token, &path(typed)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    headers(&response);
    let length = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.len(), length);
    assert!(length <= MAX_FILE_TRANSFER_MANIFEST_BYTES);
    assert_eq!(&FileTransferManifest::parse_json(&bytes).unwrap(), expected);
    assert_eq!(bytes.as_ref(), serde_json::to_vec(expected).unwrap());
    assert_eq!(f.sharing.snapshot().await, before);
}
pub async fn healthy(f: &Fixture, route: &Router) {
    success(f, route, &f.request(), &f.sharing.base.manifest).await;
}
pub async fn saturated(route: &Router, uri: &str) {
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            route,
            body_request(TOKEN, uri, unread()),
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("full HTTP admission must fail without queueing, body polling or SQL");
}

pub struct Held {
    future: Option<tokio::task::JoinHandle<Response>>,
    pub release: Arc<Semaphore>,
}
impl Held {
    pub async fn start(route: Router, token: &str, uri: &str) -> Self {
        let entered = Arc::new(Notify::new());
        let observer = entered.clone();
        let release = Arc::new(Semaphore::new(0));
        let gate = release.clone();
        let body = Body::from_stream(futures_util::stream::once(async move {
            observer.notify_one();
            gate.acquire().await.unwrap().forget();
            Ok::<_, std::io::Error>(axum::body::Bytes::new())
        }));
        let request = body_request(token, uri, body);
        let held = Self {
            future: Some(tokio::spawn(async move {
                route.oneshot(request).await.unwrap()
            })),
            release,
        };
        tokio::time::timeout(Duration::from_secs(1), entered.notified())
            .await
            .expect("actual admitted request must reach its body barrier");
        held
    }
    pub async fn cancel(mut self) {
        self.future.as_ref().unwrap().abort();
        let result = tokio::time::timeout(Duration::from_secs(3), self.future.as_mut().unwrap())
            .await
            .expect("cancelled body barrier must terminate its owned task");
        self.future.take();
        assert!(result.unwrap_err().is_cancelled());
    }
    pub async fn finish(mut self) -> Response {
        self.release.add_permits(1);
        let result = tokio::time::timeout(Duration::from_secs(15), self.future.as_mut().unwrap())
            .await
            .expect("released body barrier must finish its owned request");
        self.future.take();
        result.unwrap()
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        self.release.add_permits(1);
        if let Some(task) = &self.future {
            task.abort();
        }
    }
}

pub async fn additional(
    f: &Fixture,
    seed: u128,
    length: u64,
) -> (FileTransferManifest, SessionFileDownloadRequest) {
    let actor = f.sharing.actor();
    let manifest = f.sharing.base.stage_additional(seed, length, actor).await;
    let mut attempt = f
        .sharing
        .base
        .publisher
        .prepare(
            publication_fixture::upload(seed),
            actor.principal(),
            actor.account_id(),
        )
        .await
        .unwrap();
    auth(f)
        .source
        .publish_session_file_upload(
            &auth(f).learner_token,
            &f.sharing.base.workspace,
            grant_fixture::grant(1),
            &mut attempt,
        )
        .await
        .unwrap();
    auth(f)
        .source
        .issue_session_file_recipient_grant(
            &auth(f).learner_token,
            &f.sharing.workspace,
            &SessionFileRecipientGrantIssue {
                grant_id: sharing_fixture::recipient_grant(seed),
                publisher_grant_id: grant_fixture::grant(1),
                upload_id: publication_fixture::upload(seed),
                expected_file: FileVersionRef::from_manifest(&manifest),
                recipient: f.sharing.recipient,
                validity: grant_fixture::validity(),
            },
        )
        .await
        .unwrap();
    let request = SessionFileDownloadRequest {
        grant_id: sharing_fixture::recipient_grant(seed),
        upload_id: publication_fixture::upload(seed),
        expected_file: FileVersionRef::from_manifest(&manifest),
    };
    (manifest, request)
}
