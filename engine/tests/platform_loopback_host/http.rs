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
    pub(super) status: u16,
    headers: HeaderMap,
    value: Value,
}
pub(super) fn client() -> Client {
    Client::builder()
        .http1_only()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}
pub(super) async fn call(
    client: &Client,
    origin: &str,
    method: Method,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Reply {
    let mut request = client
        .request(method, format!("{origin}{path}"))
        .header("origin", origin)
        .header("x-cyanrex-platform-request", "1");
    let cookie = token
        .map(|value| {
            format!("cyanrex_platform_session={value}; cyanrex_session=synthetic-teaching-cookie")
        })
        .unwrap_or_else(|| "cyanrex_session=synthetic-teaching-cookie".to_owned());
    let mut cookie = HeaderValue::from_str(&cookie).unwrap();
    cookie.set_sensitive(true);
    request = request.header(header::COOKIE, cookie);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("owned loopback HTTP request failed"));
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response
        .bytes()
        .await
        .unwrap_or_else(|_| panic!("owned loopback HTTP response failed"));
    if bytes.len() > 4096 {
        panic!("loopback response exceeded fixture budget");
    }
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("loopback response was not private JSON"));
    Reply {
        status,
        headers,
        value,
    }
}
pub(super) async fn login(client: &Client, origin: &str) -> Reply {
    login_code(client, origin, &otp()).await
}
pub(super) async fn login_code(client: &Client, origin: &str, code: &str) -> Reply {
    call(client,origin,Method::POST,LOGIN,None,Some(json!({"schema_version":1,"username":username().as_str(),"password":password(),"otp":code}))).await
}
pub(super) async fn get(client: &Client, origin: &str, token: &str) -> Reply {
    call(client, origin, Method::GET, SESSION, Some(token), None).await
}
pub(super) async fn logout(client: &Client, origin: &str, token: &str) -> Reply {
    call(client, origin, Method::POST, LOGOUT, Some(token), None).await
}
fn private(reply: &Reply) -> bool {
    reply
        .headers
        .get(header::CACHE_CONTROL)
        .is_some_and(|h| h == "no-store")
        && reply
            .headers
            .get(header::PRAGMA)
            .is_some_and(|h| h == "no-cache")
        && reply
            .headers
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .is_some_and(|h| h == "nosniff")
        && reply
            .headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|h| h == "application/json")
        && [
            header::LOCATION,
            header::RETRY_AFTER,
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
        ]
        .iter()
        .all(|h| !reply.headers.contains_key(h))
}
pub(super) fn token(reply: &Reply) -> Option<String> {
    if reply.status != 200
        || !private(reply)
        || reply.value != json!({"schema_version":1,"authenticated":true})
        || reply.headers.get_all(header::SET_COOKIE).iter().count() != 1
    {
        return None;
    }
    // HTTP/1 does not carry HeaderValue's in-process sensitive flag over the wire.
    let cookie = reply.headers.get(header::SET_COOKIE)?.to_str().ok()?;
    let (pair, attributes) = cookie.split_once(';')?;
    if attributes != " Path=/platform/v1; HttpOnly; SameSite=Strict" {
        return None;
    }
    let (name, value) = pair.split_once('=')?;
    let id = uuid::Uuid::parse_str(value).ok()?;
    (name == "cyanrex_platform_session" && !id.is_nil() && id.to_string() == value)
        .then(|| value.to_owned())
}
pub(super) fn current(reply: &Reply) -> bool {
    reply.status == 200
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"authenticated":true})
}
pub(super) fn cleared(reply: &Reply) -> bool {
    reply.status == 200
        && private(reply)
        && reply.value == json!({"schema_version":1,"ok":true})
        && reply.headers.get_all(header::SET_COOKIE).iter().count() == 1
        && reply.headers.get(header::SET_COOKIE).is_some_and(|h| {
            h=="cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0"
        })
}
pub(super) fn denied(reply: &Reply, status: u16, code: &str, outcome: &str) -> bool {
    reply.status == status
        && private(reply)
        && !reply.headers.contains_key(header::SET_COOKIE)
        && reply.value == json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
}
