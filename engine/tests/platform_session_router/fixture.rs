pub(super) use super::{
    catalog_fixture::{within, Fixture},
    login_fixture::{otp, password, read_only, session_count, snapshot, username, watermark},
    login_http_helpers::{
        capture, capture_response, denied, issued_token, private, raw, request as login_request,
        untouched_body, valid_request, Reply, COOKIE,
    },
    storage_helpers::no_connections,
};
use super::{expectation_helpers::Pins, marker_helpers::MarkerPins};
use axum::{
    body::Body,
    http::{header, HeaderValue, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::{
    platform_host::open_platform_storage_expected,
    platform_http::{build_platform_session_router, PlatformSessionHttpState},
};
use serde_json::json;
use sqlx_core::{query::query, row::Row};

pub(super) const SESSION: &str = "/platform/v1/session";
pub(super) const LOGOUT: &str = "/platform/v1/session/logout";

pub(super) struct Prepared<'a> {
    inner: super::login_fixture::Prepared<'a>,
    catalog: Pins,
    markers: MarkerPins,
}

impl<'a> Prepared<'a> {
    pub(super) async fn new(owner: &'a Fixture) -> Self {
        let inner = super::login_fixture::Prepared::new(owner).await;
        // An explicit synthetic combined-role grant, never a permission granted by the router.
        owner
            .sql(&format!(
                "GRANT DELETE ON {}.sessions TO {}",
                owner.source, owner.role
            ))
            .await;
        let catalog = Pins::read(owner).await;
        let markers = MarkerPins::read(owner).await;
        Self {
            inner,
            catalog,
            markers,
        }
    }
    pub(super) fn root(&self) -> &super::storage_helpers::Root {
        self.inner.root()
    }
    pub(super) async fn state(&self, login: usize, lifecycle: usize) -> PlatformSessionHttpState {
        let f = self.inner.owner;
        // Exactly one factory result is consumed once. Neither lane gets a separately observed
        // binding, pool fallback or a second root/catalog baseline through a fixture shortcut.
        let storage = within(open_platform_storage_expected(
            f.config().clone(),
            super::storage_helpers::endpoint(f),
            super::storage_helpers::PASSWORD,
            self.markers.expectation(f, self.catalog),
            self.root().path(),
            self.root().original(f.config()),
        ))
        .await
        .unwrap_or_else(|_| panic!("owned declared session-router binding rejected"));
        PlatformSessionHttpState::with_capacities(storage, login, lifecycle)
            .unwrap_or_else(|_| panic!("owned session-router capacities rejected"))
    }
    pub(super) async fn app(&self) -> Router {
        build_platform_session_router(self.state(4, 4).await)
    }
    pub(super) async fn minimal_acl(&self) -> bool {
        let f = self.inner.owner;
        let row = within(query("SELECT
            count(*) FILTER(WHERE has_column_privilege($1,c.oid,a.attnum,'SELECT'))=25 AS selects,
            count(*) FILTER(WHERE has_column_privilege($1,c.oid,a.attnum,'INSERT'))=4 AS inserts,
            count(*) FILTER(WHERE has_column_privilege($1,c.oid,a.attnum,'UPDATE'))=4 AS updates,
            bool_and(NOT has_table_privilege($1,c.oid,'INSERT')
                AND NOT has_table_privilege($1,c.oid,'UPDATE')
                AND NOT has_table_privilege($1,c.oid,'TRIGGER')
                AND (has_table_privilege($1,c.oid,'DELETE')=(n.nspname=$3 AND c.relname='sessions'))) AS writes
            FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
            JOIN pg_catalog.pg_attribute a ON a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped
            WHERE n.nspname=ANY($2) AND c.relkind='r'")
            .bind(&f.role).bind(vec![f.source.clone(),f.task.clone(),f.artifacts.clone()])
            .bind(&f.source).fetch_one(f.owner())).await.unwrap();
        row.get::<bool, _>("selects")
            && row.get::<bool, _>("inserts")
            && row.get::<bool, _>("updates")
            && row.get::<bool, _>("writes")
            && self.catalog == Pins::read(f).await
            && self.markers == MarkerPins::read(f).await
    }
    pub(super) fn finish(self) -> bool {
        self.inner.finish()
    }
}

pub(super) fn lifecycle(method: &str, path: &str, token: &str, body: Body) -> Request<Body> {
    let mut cookie = HeaderValue::from_str(&format!(
        "{COOKIE}={token}; cyanrex_session=synthetic-teaching-cookie"
    ))
    .unwrap_or_else(|_| panic!("owned session-router Cookie construction failed"));
    cookie.set_sensitive(true);
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-cyanrex-platform-request", "1")
        .header(header::ORIGIN, super::login_http_helpers::ORIGIN)
        .header(header::COOKIE, cookie)
        .body(body)
        .unwrap_or_else(|_| panic!("owned lifecycle request construction failed"))
}
pub(super) fn get(token: &str) -> Request<Body> {
    lifecycle("GET", SESSION, token, Body::empty())
}
pub(super) fn logout(token: &str) -> Request<Body> {
    lifecycle("POST", LOGOUT, token, Body::empty())
}
pub(super) fn current(reply: &Reply) -> bool {
    reply.status == StatusCode::OK
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"authenticated":true})
}
pub(super) fn cleared(reply: &Reply) -> bool {
    reply.status == StatusCode::OK && private(reply)
        && reply.value == json!({"schema_version":1,"ok":true})
        && reply.headers.get_all(header::SET_COOKIE).iter().count()==1
        && reply.headers.get(header::SET_COOKIE).is_some_and(|h| h ==
            "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure")
}
pub(super) fn header_token(response: &Response) -> Option<String> {
    if response.status() != StatusCode::OK {
        return None;
    }
    let h = response.headers().get(header::SET_COOKIE)?;
    if !h.is_sensitive() {
        return None;
    }
    let pair = h.to_str().ok()?.split(';').next()?;
    let (name, token) = pair.split_once('=')?;
    if name != COOKIE {
        return None;
    }
    let id = uuid::Uuid::parse_str(token).ok()?;
    (!id.is_nil() && id.to_string() == token).then(|| token.to_owned())
}
