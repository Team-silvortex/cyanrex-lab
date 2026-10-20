use super::fixture::{otp, password, username};
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Method,
};
use serde_json::{json, Value};
use std::time::Duration;
pub(super) const LOGIN: &str = "/platform/v1/session/login";
pub(super) const SESSION: &str = "/platform/v1/session";
pub(super) const LOGOUT: &str = "/platform/v1/session/logout";
pub(super) struct Reply {
    status: u16,
    headers: HeaderMap,
    value: Value,
}
pub(super) fn client(ca: Option<&[u8]>) -> Client {
    let mut builder = Client::builder()
        .http1_only()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10));
    if let Some(ca) = ca {
        builder = builder.add_root_certificate(reqwest::Certificate::from_pem(ca).unwrap());
    }
    builder.build().unwrap()
}
pub(super) fn wrong_peer_client(ca: &[u8], bind: std::net::SocketAddr) -> Client {
    Client::builder()
        .http1_only()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10))
        .add_root_certificate(reqwest::Certificate::from_pem(ca).unwrap())
        .resolve("wrong-platform-peer.test", bind)
        .build()
        .unwrap()
}
fn request(
    client: &Client,
    origin: &str,
    method: Method,
    path: &str,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut cookie = HeaderValue::from_str(
        &token
            .map(|v| {
                format!("cyanrex_platform_session={v}; cyanrex_session=synthetic-teaching-cookie")
            })
            .unwrap_or_else(|| "cyanrex_session=synthetic-teaching-cookie".into()),
    )
    .unwrap();
    cookie.set_sensitive(true);
    client
        .request(method, format!("{origin}{path}"))
        .header("origin", origin)
        .header("x-cyanrex-platform-request", "1")
        .header(header::COOKIE, cookie)
}
pub(super) async fn refused_login(client: &Client, origin: &str) -> bool {
    request(client,origin,Method::POST,LOGIN,None).json(&json!({"schema_version":1,"username":username().as_str(),"password":password(),"otp":otp()})).send().await.is_err()
}
pub(super) async fn login(client: &Client, origin: &str) -> Reply {
    capture(request(client,origin,Method::POST,LOGIN,None).json(&json!({"schema_version":1,"username":username().as_str(),"password":password(),"otp":otp()}))).await
}
pub(super) async fn get(client: &Client, origin: &str, token: &str) -> Reply {
    capture(request(client, origin, Method::GET, SESSION, Some(token))).await
}
pub(super) async fn logout(client: &Client, origin: &str, token: &str) -> Reply {
    capture(request(client, origin, Method::POST, LOGOUT, Some(token))).await
}
async fn capture(request: reqwest::RequestBuilder) -> Reply {
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("owned HTTPS request failed"));
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response
        .bytes()
        .await
        .unwrap_or_else(|_| panic!("owned HTTPS body failed"));
    assert!(
        bytes.len() <= 4096,
        "owned HTTPS response exceeded fixture budget"
    );
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("owned HTTPS response was not private JSON"));
    Reply {
        status,
        headers,
        value,
    }
}
fn private(reply: &Reply) -> bool {
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
        .all(|v| !reply.headers.contains_key(v))
}
pub(super) fn token(reply: &Reply) -> Option<String> {
    if reply.status != 200
        || !private(reply)
        || reply.value != json!({"schema_version":1,"authenticated":true})
        || reply.headers.get_all(header::SET_COOKIE).iter().count() != 1
    {
        return None;
    }
    // HeaderValue sensitivity is not transported over HTTP/1; inspect only real wire policy.
    let (pair, attrs) = reply
        .headers
        .get(header::SET_COOKIE)?
        .to_str()
        .ok()?
        .split_once(';')?;
    let (name, value) = pair.split_once('=')?;
    let id = uuid::Uuid::parse_str(value).ok()?;
    (name == "cyanrex_platform_session"
        && attrs == " Path=/platform/v1; HttpOnly; SameSite=Strict; Secure"
        && !id.is_nil()
        && id.to_string() == value)
        .then(|| value.to_owned())
}
pub(super) fn current(reply: &Reply) -> bool {
    reply.status == 200
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"authenticated":true})
}
pub(super) fn cleared(reply: &Reply) -> bool {
    reply.status==200&&private(reply)&&reply.value==json!({"schema_version":1,"ok":true})&&reply.headers.get_all(header::SET_COOKIE).iter().count()==1&&reply.headers.get(header::SET_COOKIE).is_some_and(|v|v=="cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure")
}
pub(super) fn denied(reply: &Reply, status: u16, code: &str, outcome: &str) -> bool {
    reply.status == status
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
}
