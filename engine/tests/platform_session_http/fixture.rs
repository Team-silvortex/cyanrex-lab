use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::{AuthorityId, LegacyUsername},
    platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState},
    services::auth_service::durable_source::DurableAuthSource,
};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

pub const ORIGIN: &str = "https://platform.example";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
pub const SESSION: &str = "/platform/v1/session";
pub const LOGOUT: &str = "/platform/v1/session/logout";
pub const PASSWORD: &str = "synthetic-session-http-password";
const DEADLINE: Duration = Duration::from_secs(15);
type Case<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub fn authority() -> AuthorityId {
    "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap()
}

pub fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn otp(secret: &str) -> String {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(&data_encoding::BASE32.decode(secret.as_bytes()).unwrap())
            .unwrap();
    mac.update(&(Utc::now().timestamp().div_euclid(30) as u64).to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 15) as usize;
    let number = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", number % 1_000_000)
}

pub async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(DEADLINE, future)
        .await
        .unwrap_or_else(|_| panic!("bounded Session HTTP fixture operation timed out"))
}

// Neither credentials nor response contents implement Debug in this fixture.
pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub value: Value,
}

pub fn request(method: &str, path: &str, token: &str) -> Request<Body> {
    let mut cookie = HeaderValue::from_str(&format!("{COOKIE}={token}"))
        .unwrap_or_else(|_| panic!("synthetic Cookie construction failed"));
    cookie.set_sensitive(true);
    Request::builder()
        .method(method)
        .uri(path)
        .header(MARKER, "1")
        .header("origin", ORIGIN)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap_or_else(|_| panic!("synthetic Session HTTP request construction failed"))
}

pub async fn capture(app: Router, request: Request<Body>) -> Response {
    let response = within(app.oneshot(request))
        .await
        .unwrap_or_else(|_| panic!("Session HTTP dispatch failed"));
    capture_response(response).await
}

pub async fn capture_response(response: axum::response::Response) -> Response {
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = within(response.into_body().collect())
        .await
        .unwrap_or_else(|_| panic!("Session HTTP response body failed"))
        .to_bytes();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("Session HTTP response was not JSON"));
    Response {
        status,
        headers,
        value,
    }
}

pub fn common_headers(response: &Response) {
    assert_eq!(response.headers.get("cache-control").unwrap(), "no-store");
    assert_eq!(response.headers.get("pragma").unwrap(), "no-cache");
    assert_eq!(
        response.headers.get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        response.headers.get("content-type").unwrap(),
        "application/json"
    );
    for name in ["retry-after", "location", "access-control-allow-origin"] {
        assert!(!response.headers.contains_key(name));
    }
}

pub fn private_headers(response: &Response, clear: bool) {
    common_headers(response);
    assert_eq!(
        response.headers.get_all("set-cookie").iter().count(),
        usize::from(clear)
    );
    if clear {
        assert_eq!(response.headers.get("set-cookie").unwrap(),
            "cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure");
    }
}

pub fn successful(response: &Response, value: Value, clear: bool) {
    assert_eq!(response.status, StatusCode::OK);
    private_headers(response, clear);
    assert!(
        response.value == value,
        "Session HTTP projection changed or exposed extra fields"
    );
}

pub fn denied(response: &Response, status: StatusCode, code: &str, outcome: &str) {
    assert_eq!(response.status, status);
    private_headers(response, false);
    assert!(
        response.value
            == serde_json::json!({"schema_version":1,
        "error":{"code":code,"outcome":outcome}}),
        "private Session error projection changed"
    );
}

pub struct Snapshot {
    pub users: String,
    pub source: String,
    pub sessions: String,
    pub session_count: i64,
    pub registry_absent: bool,
    pub namespace: i64,
    pub relations: Vec<i64>,
}

pub fn unchanged(before: &Snapshot, after: &Snapshot) -> bool {
    before.users == after.users
        && before.source == after.source
        && before.sessions == after.sessions
        && before.session_count == after.session_count
        && before.registry_absent == after.registry_absent
        && before.namespace == after.namespace
        && before.relations == after.relations
}

pub struct Pending<T> {
    job: Option<tokio::task::JoinHandle<T>>,
}

impl<T: Send + 'static> Pending<T> {
    pub fn start(future: impl Future<Output = T> + Send + 'static) -> Self {
        Self {
            job: Some(tokio::spawn(future)),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.job.as_ref().unwrap().is_finished()
    }

    pub async fn finish(mut self) -> T {
        let result = within(self.job.as_mut().unwrap()).await;
        self.job.take();
        result.unwrap_or_else(|_| panic!("owned Session HTTP task failed"))
    }

    pub async fn cancel(mut self) -> bool {
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

pub struct Fixture {
    pub schema: String,
    pub token: String,
    pub other_token: String,
    pub username: LegacyUsername,
    pub account_id: Uuid,
    admin: Option<PgPool>,
    pool: Option<PgPool>,
    source: Option<DurableAuthSource>,
    namespaces: Vec<String>,
}

impl Fixture {
    async fn empty() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("set an explicit disposable CYANREX_TEST_DATABASE_URL");
        let admin = within(PgPoolOptions::new().max_connections(2).connect(&url))
            .await
            .unwrap_or_else(|_| panic!("disposable fixture database unavailable"));
        Self {
            schema: format!("session_http_{}", Uuid::new_v4().simple()),
            token: String::new(),
            other_token: String::new(),
            username: "session-http-learner".parse().unwrap(),
            account_id: Uuid::nil(),
            admin: Some(admin),
            pool: None,
            source: None,
            namespaces: Vec::new(),
        }
    }

    async fn prepare(&mut self) {
        self.namespaces.push(self.schema.clone());
        within(query(&format!("CREATE SCHEMA {}", self.schema)).execute(self.admin()))
            .await
            .unwrap_or_else(|_| panic!("fresh fixture namespace creation failed"));
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let schema = self.schema.clone();
        let pool = within(PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                    .bind(schema).execute(connection).await?;
                Ok(())
            })
        }).connect(&url)).await.unwrap_or_else(|_| panic!("scoped fixture pool unavailable"));
        self.source = Some(DurableAuthSource::new(pool.clone(), authority()));
        self.pool = Some(pool);
        within(self.source().install_empty_schema())
            .await
            .unwrap_or_else(|_| panic!("fresh Source2 installation failed"));
        let registration = within(self.source().register(&self.username, PASSWORD))
            .await
            .unwrap_or_else(|_| panic!("synthetic fixture registration failed"));
        self.account_id = registration.account.account_id.as_uuid();
        let login = within(self.source().login(
            &self.username,
            PASSWORD,
            &otp(&registration.bootstrap.secret),
        ))
        .await
        .unwrap_or_else(|_| panic!("synthetic fixture login failed"));
        self.token = login.token;
        // A separate synthetic device Session, not evidence of a second login/OTP consumption.
        self.other_token = Uuid::new_v4().to_string();
        within(
            query(
                "INSERT INTO sessions (token, username, account_id, expires_at)
            VALUES ($1, $2, $3, clock_timestamp() + INTERVAL '12 hours')",
            )
            .bind(token_hash(&self.other_token))
            .bind(self.username.as_str())
            .bind(registration.account.account_id.as_uuid())
            .execute(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("synthetic second device Session setup failed"));
    }

    pub fn source(&self) -> &DurableAuthSource {
        self.source.as_ref().unwrap()
    }
    pub fn pool(&self) -> &PgPool {
        self.pool.as_ref().unwrap()
    }
    pub fn admin(&self) -> &PgPool {
        self.admin.as_ref().unwrap()
    }

    pub fn app(&self) -> Router {
        build_session_lifecycle_router(
            SessionLifecycleHttpState::new(self.source().clone(), ORIGIN)
                .unwrap_or_else(|_| panic!("trusted Session HTTP state rejected")),
        )
    }

    pub fn app_with_capacity(&self, capacity: usize) -> Router {
        build_session_lifecycle_router(
            SessionLifecycleHttpState::with_capacity(self.source().clone(), ORIGIN, capacity)
                .unwrap_or_else(|_| panic!("trusted Session HTTP capacity rejected")),
        )
    }

    pub async fn sql(&self, statement: &str) {
        within(query(statement).execute(self.pool()))
            .await
            .unwrap_or_else(|_| panic!("owned synthetic SQL fixture setup failed"));
    }

    pub async fn shadow(&mut self) -> String {
        let schema = format!("session_http_shadow_{}", Uuid::new_v4().simple());
        self.namespaces.push(schema.clone());
        within(query(&format!("CREATE SCHEMA {schema}")).execute(self.admin()))
            .await
            .unwrap_or_else(|_| panic!("owned shadow namespace setup failed"));
        within(
            query(&format!(
                "CREATE TABLE {schema}.sessions (token TEXT PRIMARY KEY)"
            ))
            .execute(self.admin()),
        )
        .await
        .unwrap_or_else(|_| panic!("owned shadow table setup failed"));
        schema
    }

    pub async fn fault(&self, timing: &str, body: &str) {
        self.sql("CREATE SEQUENCE session_http_calls").await;
        self.sql(&format!(
            "CREATE FUNCTION session_http_fault() RETURNS trigger
            LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.session_http_calls'); {body} END $$",
            self.schema
        ))
        .await;
        self.sql(&format!(
            "CREATE TRIGGER session_http_fault {timing} ON sessions
            FOR EACH ROW EXECUTE FUNCTION session_http_fault()"
        ))
        .await;
    }

    pub async fn calls(&self) -> (bool, i64) {
        let row = within(
            query("SELECT last_value, is_called FROM session_http_calls").fetch_one(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("fault counter unavailable"));
        (row.get("is_called"), row.get("last_value"))
    }

    pub async fn blocked_by(&self, blocker: i32) -> i32 {
        within(async {
            loop {
                let row = query(
                    "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                    AND application_name = $1 AND $2 = ANY(pg_blocking_pids(pid))
                    ORDER BY pid LIMIT 1",
                )
                .bind(&self.schema)
                .bind(blocker)
                .fetch_optional(self.admin())
                .await
                .unwrap_or_else(|_| panic!("actual SQL blocker observation failed"));
                if let Some(row) = row {
                    return row.get("pid");
                }
                // Poll an independently observed database predicate, not a timing claim.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
    }

    pub async fn drain(&self) -> bool {
        within(async {
            let mut held = Vec::new();
            let mut restored = true;
            for _ in 0..4 {
                let mut connection = self
                    .pool()
                    .acquire()
                    .await
                    .unwrap_or_else(|_| panic!("source pool did not drain"));
                let row = query(
                    "SELECT current_schema()::text = $1
                    AND current_setting('search_path') = $1 AS restored",
                )
                .bind(&self.schema)
                .fetch_one(&mut *connection)
                .await
                .unwrap_or_else(|_| panic!("source pool path observation failed"));
                restored &= row.get::<bool, _>("restored");
                held.push(connection);
            }
            restored
        })
        .await
    }

    pub async fn snapshot(&self) -> Snapshot {
        let digest = |table: &str| {
            format!("SELECT md5(COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text) AS digest FROM {table} t")
        };
        let mut values = Vec::new();
        for table in ["users", "collaboration_auth_source_schema", "sessions"] {
            let value = within(query(&digest(table)).fetch_one(self.pool()))
                .await
                .unwrap_or_else(|_| panic!("fixture snapshot failed"));
            values.push(value.get::<String, _>("digest"));
        }
        let row = within(
            query(
                "SELECT (SELECT count(*) FROM sessions) AS sessions,
            to_regclass('collaboration_identity_schema') IS NULL AS registry_absent,
            current_schema()::regnamespace::oid::bigint AS namespace",
            )
            .fetch_one(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("fixture count failed"));
        let relations = within(
            query(
                "SELECT oid::bigint AS oid FROM pg_class
            WHERE relnamespace = current_schema()::regnamespace AND relname IN
            ('users','sessions','collaboration_auth_source_schema') ORDER BY relname",
            )
            .fetch_all(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("relation snapshot failed"))
        .into_iter()
        .map(|row| row.get("oid"))
        .collect();
        Snapshot {
            users: values.remove(0),
            source: values.remove(0),
            sessions: values.remove(0),
            session_count: row.get("sessions"),
            registry_absent: row.get("registry_absent"),
            namespace: row.get("namespace"),
            relations,
        }
    }

    async fn cleanup(&mut self) {
        self.source.take();
        if let Some(pool) = &self.pool {
            within(pool.close()).await;
        }
        self.pool.take();
        if let Some(admin) = &self.admin {
            for schema in self.namespaces.iter().rev() {
                within(query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).execute(admin))
                    .await
                    .unwrap_or_else(|_| panic!("owned fixture namespace cleanup failed"));
            }
            within(admin.close()).await;
        }
        self.admin.take();
    }

    pub async fn checked<T>(case: impl for<'a> FnOnce(&'a mut Self) -> Case<'a, T>) -> T {
        use futures_util::FutureExt;
        let mut fixture = Self::empty().await;
        let result = AssertUnwindSafe(async {
            fixture.prepare().await;
            case(&mut fixture).await
        })
        .catch_unwind()
        .await;
        let cleanup = AssertUnwindSafe(fixture.cleanup()).catch_unwind().await;
        if let Err(panic) = cleanup {
            std::panic::resume_unwind(panic);
        }
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}
