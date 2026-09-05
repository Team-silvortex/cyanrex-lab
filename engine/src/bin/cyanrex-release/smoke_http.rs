use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderValue, COOKIE, ORIGIN, SET_COOKIE};
use reqwest::{Client, Response, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;
use sha1::Sha1;
use std::net::IpAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const SESSION_COOKIE: &str = "cyanrex_session=";
const TOTP_STEP_SECONDS: u64 = 30;

pub struct SmokeHttpClient {
    client: Client,
    base_url: Url,
    origin: HeaderValue,
    cookie: HeaderValue,
}

impl SmokeHttpClient {
    pub async fn public_get_json<T: DeserializeOwned>(
        engine_url: &str,
        path: &str,
    ) -> Result<T, String> {
        let base_url = validate_engine_url(engine_url)?;
        let response = http_client()?
            .get(endpoint(&base_url, path)?)
            .send()
            .await
            .map_err(|error| format!("GET {path} failed: {error}"))?;
        response_json(path, response).await
    }

    pub async fn login(
        engine_url: &str,
        origin: &str,
        password: &str,
        totp_secret: &str,
    ) -> Result<Self, String> {
        let base_url = validate_engine_url(engine_url)?;
        let origin = validate_origin(origin)?;
        let client = http_client()?;
        let otp = current_totp(totp_secret)?;
        let response = client
            .post(endpoint(&base_url, "/auth/login")?)
            .json(&json!({"username": "admin", "password": password, "otp": otp}))
            .send()
            .await
            .map_err(|error| format!("admin login request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(response_error("admin login", response).await);
        }
        let cookie = session_cookie(&response)?;
        Ok(Self {
            client,
            base_url,
            origin,
            cookie,
        })
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        let response = self
            .client
            .get(endpoint(&self.base_url, path)?)
            .header(COOKIE, &self.cookie)
            .query(query)
            .send()
            .await
            .map_err(|error| format!("GET {path} failed: {error}"))?;
        response_json(path, response).await
    }

    pub async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let response = self
            .client
            .post(endpoint(&self.base_url, path)?)
            .header(COOKIE, &self.cookie)
            .header(ORIGIN, &self.origin)
            .json(body)
            .send()
            .await
            .map_err(|error| format!("POST {path} failed: {error}"))?;
        response_json(path, response).await
    }
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("cannot initialize smoke HTTP client: {error}"))
}

fn validate_engine_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("Engine URL is invalid: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "Engine URL must be an HTTP(S) origin without credentials or a path".to_owned(),
        );
    }
    if url.scheme() == "http" && !loopback_host(&url) {
        return Err("plaintext Engine smoke URLs must use a loopback host".to_owned());
    }
    Ok(url)
}

fn validate_origin(value: &str) -> Result<HeaderValue, String> {
    let url = Url::parse(value).map_err(|error| format!("smoke Origin is invalid: {error}"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "smoke Origin must be an HTTP(S) origin without credentials or a path".to_owned(),
        );
    }
    HeaderValue::from_str(value).map_err(|error| format!("smoke Origin is invalid: {error}"))
}

fn loopback_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn endpoint(base_url: &Url, path: &str) -> Result<Url, String> {
    base_url
        .join(path.trim_start_matches('/'))
        .map_err(|error| format!("cannot build Engine endpoint {path}: {error}"))
}

fn session_cookie(response: &Response) -> Result<HeaderValue, String> {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .filter_map(|header| header.split(';').next())
        .find(|cookie| cookie.starts_with(SESSION_COOKIE) && cookie.len() > SESSION_COOKIE.len())
        .ok_or_else(|| "admin login did not return a Cyanrex session cookie".to_owned())
        .and_then(|cookie| {
            HeaderValue::from_str(cookie)
                .map_err(|error| format!("admin login returned an invalid session cookie: {error}"))
        })
}

async fn response_json<T: DeserializeOwned>(label: &str, response: Response) -> Result<T, String> {
    let status = response.status();
    let source = limited_response(label, response).await?;
    if !status.is_success() {
        return Err(format!(
            "{label} returned HTTP {status}: {}",
            response_excerpt(&source)
        ));
    }
    serde_json::from_slice(&source)
        .map_err(|error| format!("{label} returned invalid JSON: {error}"))
}

async fn response_error(label: &str, response: Response) -> String {
    let status = response.status();
    match limited_response(label, response).await {
        Ok(source) => format!(
            "{label} returned HTTP {status}: {}",
            response_excerpt(&source)
        ),
        Err(error) => format!("{label} returned HTTP {status}: {error}"),
    }
}

async fn limited_response(label: &str, mut response: Response) -> Result<Vec<u8>, String> {
    let mut source = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("cannot read {label} response: {error}"))?
    {
        if source
            .len()
            .checked_add(chunk.len())
            .is_none_or(|size| size > MAX_RESPONSE_BYTES)
        {
            return Err(format!(
                "{label} response exceeds the {MAX_RESPONSE_BYTES}-byte limit"
            ));
        }
        source.extend_from_slice(&chunk);
    }
    Ok(source)
}

fn response_excerpt(source: &[u8]) -> String {
    let text = String::from_utf8_lossy(&source[..source.len().min(512)]);
    text.replace(['\r', '\n'], " ").trim().to_owned()
}

fn current_totp(secret: &str) -> Result<String, String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock predates the Unix epoch".to_owned())?
        .as_secs();
    totp(secret, seconds / TOTP_STEP_SECONDS)
}

fn totp(secret: &str, counter: u64) -> Result<String, String> {
    let normalized = secret
        .trim()
        .to_ascii_uppercase()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let key = BASE32
        .decode(normalized.as_bytes())
        .map_err(|_| "CYANREX_ADMIN_TOTP_SECRET is not valid base32".to_owned())?;
    if key.is_empty() {
        return Err("CYANREX_ADMIN_TOTP_SECRET must not be empty".to_owned());
    }
    let mut mac = Hmac::<Sha1>::new_from_slice(&key).expect("HMAC accepts arbitrary key lengths");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | digest[offset + 3] as u32;
    Ok(format!("{:06}", binary % 1_000_000))
}

#[cfg(test)]
mod tests {
    use super::{totp, validate_engine_url};

    #[test]
    fn totp_matches_the_rfc_sha1_vector_truncated_to_six_digits() {
        assert_eq!(
            totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1).unwrap(),
            "287082"
        );
    }

    #[test]
    fn admin_credentials_require_https_or_loopback() {
        assert!(validate_engine_url("http://127.0.0.1:8080").is_ok());
        assert!(validate_engine_url("https://engine.example.test").is_ok());
        assert!(validate_engine_url("http://engine.example.test").is_err());
        assert!(validate_engine_url("https://user@engine.example.test/path").is_err());
    }
}
