pub(super) use super::{
    catalog_fixture::{within, Fixture},
    login_fixture::{
        otp, password, read_only, seed_session, session_count, snapshot, username, watermark,
        Prepared,
    },
    storage_helpers::no_connections,
};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderMap, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::platform_http::{build_session_login_router, SessionLoginHttpState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::{future::Future, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

pub(super) const ORIGIN: &str = "https://platform.example";
pub(super) const LOGIN: &str = "/platform/v1/session/login";
pub(super) const COOKIE: &str = "cyanrex_platform_session";

// Never derive Debug for credentials, response headers, tokens or body projections.
pub(super) struct Reply {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) value: Value,
}

pub(super) async fn state(prepared: &Prepared<'_>, capacity: usize) -> SessionLoginHttpState {
    SessionLoginHttpState::with_capacity(prepared.connector().await, capacity)
        .unwrap_or_else(|_| panic!("owned login HTTP state rejected"))
}

pub(super) async fn app(prepared: &Prepared<'_>) -> Router {
    build_session_login_router(state(prepared, 4).await)
}

pub(super) fn request(name: &str, secret: &str, code: &str) -> Request<Body> {
    raw(Body::from(
        serde_json::to_vec(&json!({"schema_version":1,"username":name,
            "password":secret,"otp":code}))
        .unwrap_or_else(|_| panic!("synthetic login JSON construction failed")),
    ))
}

pub(super) fn valid_request() -> Request<Body> {
    request(username().as_str(), password(), &otp())
}

pub(super) fn raw(body: Body) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(LOGIN)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap_or_else(|_| panic!("synthetic login request construction failed"))
}

pub(super) fn untouched_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
            panic!("capacity rejection polled a request body")
        },
    ))
}

pub(super) async fn capture(app: Router, request: Request<Body>) -> Reply {
    let response = within(app.oneshot(request))
        .await
        .unwrap_or_else(|_| panic!("owned login HTTP dispatch failed"));
    capture_response(response).await
}

pub(super) async fn capture_response(response: Response) -> Reply {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = within(response.into_body().collect())
        .await
        .unwrap_or_else(|_| panic!("owned login HTTP response body failed"))
        .to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("owned login HTTP response was not JSON"));
    Reply {
        status,
        headers,
        value,
    }
}

pub(super) fn private(reply: &Reply) -> bool {
    reply
        .headers
        .get(header::CACHE_CONTROL)
        .is_some_and(|v| v == "no-store")
        && reply
            .headers
            .get(header::PRAGMA)
            .is_some_and(|v| v == "no-cache")
        && reply
            .headers
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .is_some_and(|v| v == "nosniff")
        && reply
            .headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v == "application/json")
        && [
            header::LOCATION,
            header::RETRY_AFTER,
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
        ]
        .iter()
        .all(|name| !reply.headers.contains_key(name))
}

pub(super) fn denied(reply: &Reply, status: StatusCode, code: &str, outcome: &str) -> bool {
    reply.status == status
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
}

pub(super) fn issued_token(reply: &Reply) -> Option<String> {
    if reply.status != StatusCode::OK
        || !private(reply)
        || reply.value != json!({"schema_version":1,"authenticated":true})
        || reply.headers.get_all(header::SET_COOKIE).iter().count() != 1
    {
        return None;
    }
    let header = reply.headers.get(header::SET_COOKIE)?;
    if !header.is_sensitive() {
        return None;
    }
    let cookie = header.to_str().ok()?;
    let (pair, attributes) = cookie.split_once(';')?;
    let (name, token) = pair.split_once('=')?;
    if name != COOKIE
        || attributes != " Path=/platform/v1; HttpOnly; SameSite=Strict; Secure"
        || token.len() != 36
        || !Uuid::parse_str(token).is_ok_and(|id| !id.is_nil() && id.to_string() == token)
    {
        return None;
    }
    Some(token.to_owned())
}

pub(super) async fn stored_only_as_digest(f: &Fixture, token: &str) -> bool {
    let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
    within(
        query(&format!(
            "SELECT count(*) FILTER (WHERE token=$1 AND username=$2
        AND account_id='00000000-0000-0000-0000-000000008211'
        AND expires_at>clock_timestamp() AND expires_at<clock_timestamp()+interval '13 hours')=1
        AND count(*) FILTER (WHERE token=$3)=0 AS exact FROM {}.sessions",
            f.source
        ))
        .bind(digest)
        .bind(username().as_str())
        .bind(token)
        .fetch_one(f.owner()),
    )
    .await
    .unwrap()
    .get("exact")
}

pub(super) async fn source_holder(f: &Fixture) -> (PgConnection, i32) {
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query(&format!(
            "SELECT singleton FROM {}.collaboration_auth_source_schema FOR UPDATE",
            f.source
        ))
        .fetch_one(&mut connection),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, pid)
}

pub(super) async fn waiting_on(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pending: bool = query(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity
                WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection'
                AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS pending",
            )
            .bind(&f.database)
            .bind(&f.role)
            .bind(blocker)
            .fetch_one(f.owner())
            .await
            .unwrap()
            .get("pending");
            if pending {
                break true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

pub(super) async fn release(mut connection: PgConnection) -> bool {
    let rolled_back = within(query("ROLLBACK").execute(&mut connection))
        .await
        .is_ok();
    let closed = within(connection.close()).await.is_ok();
    rolled_back && closed
}

pub(super) struct Pending<T> {
    job: Option<tokio::task::JoinHandle<T>>,
}
impl<T: Send + 'static> Pending<T> {
    pub(super) fn start(future: impl Future<Output = T> + Send + 'static) -> Self {
        Self {
            job: Some(tokio::spawn(future)),
        }
    }
    pub(super) async fn cancel(mut self) -> bool {
        self.job.as_ref().unwrap().abort();
        let result = within(self.job.as_mut().unwrap()).await;
        self.job.take();
        result.is_err_and(|error| error.is_cancelled())
    }
}
impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}
