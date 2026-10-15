use crate::{
    models::collaboration::{AuthorityId, LegacyUsername},
    services::auth_service::durable_source::DurableAuthSource,
    sqlx_compat::{query, PgPool, PgPoolOptions, Row},
};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;

#[path = "fixture_control.rs"]
mod control;
pub(super) use control::Pending;

pub(super) const PASSWORD: &str = "synthetic-session-stock-password";
// A synthetic RFC-style fixture key, never a deployed enrollment secret.
const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const INITIAL_TIME: i64 = 1_800_000_000;
type Case<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(super) async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .unwrap_or_else(|_| panic!("bounded Session stock fixture operation timed out"))
}

pub(super) struct Snapshot {
    digests: Vec<String>,
    pins: Vec<i64>,
    pub(super) sessions: i64,
    pub(super) watermark: i64,
}

pub(super) fn unchanged(before: &Snapshot, after: &Snapshot) -> bool {
    before.digests == after.digests
        && before.sessions == after.sessions
        && before.watermark == after.watermark
        && before.pins == after.pins
}

pub(super) struct Fixture {
    schema: String,
    pub(super) first: LegacyUsername,
    authority: AuthorityId,
    clock: Arc<AtomicI64>,
    admin: Option<PgPool>,
    pool: Option<PgPool>,
    source: Option<DurableAuthSource>,
    namespaces: Vec<String>,
    pools: Vec<PgPool>,
}

impl Fixture {
    async fn empty() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("set an explicit disposable CYANREX_TEST_DATABASE_URL");
        let admin = within(PgPoolOptions::new().max_connections(2).connect(&url))
            .await
            .unwrap_or_else(|_| panic!("disposable Session stock database unavailable"));
        Self {
            schema: format!("session_stock_{}", Uuid::new_v4().simple()),
            first: "stock-first-account".parse().unwrap(),
            authority: Uuid::new_v4().to_string().parse().unwrap(),
            clock: Arc::new(AtomicI64::new(INITIAL_TIME)),
            admin: Some(admin),
            pool: None,
            source: None,
            namespaces: Vec::new(),
            pools: Vec::new(),
        }
    }

    async fn prepare(&mut self) {
        self.namespaces.push(self.schema.clone());
        within(query(&format!("CREATE SCHEMA {}", self.schema)).execute(self.admin()))
            .await
            .unwrap_or_else(|_| panic!("fresh Session stock namespace creation failed"));
        let schema = self.schema.clone();
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let pool = within(PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                    .bind(schema).execute(connection).await?;
                Ok(())
            })
        }).connect(&url)).await
            .unwrap_or_else(|_| panic!("scoped Session stock pool unavailable"));
        let mut source = DurableAuthSource::new(pool.clone(), self.authority);
        let clock = self.clock.clone();
        source.otp_clock = Some(Arc::new(move || {
            chrono::DateTime::from_timestamp(clock.load(Ordering::SeqCst), 0).unwrap()
        }));
        self.pool = Some(pool);
        self.source = Some(source);
        within(self.source().install_empty_schema())
            .await
            .unwrap_or_else(|_| panic!("fresh Source2 stock fixture installation failed"));
        self.register_name(&self.first).await;
    }

    pub(super) async fn register_name(&self, name: &LegacyUsername) {
        within(self.source().register(name, PASSWORD))
            .await
            .unwrap_or_else(|_| panic!("real stock fixture registration failed"));
        within(
            query("UPDATE users SET totp_secret = $1 WHERE username = $2")
                .bind(SECRET)
                .bind(name.as_str())
                .execute(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("synthetic deterministic OTP key setup failed"));
    }

    pub(super) fn source(&self) -> &DurableAuthSource {
        self.source.as_ref().unwrap()
    }
    pub(super) fn pool(&self) -> &PgPool {
        self.pool.as_ref().unwrap()
    }
    pub(super) fn admin(&self) -> &PgPool {
        self.admin.as_ref().unwrap()
    }

    pub(super) fn code(&self) -> String {
        let mut mac =
            Hmac::<Sha1>::new_from_slice(&data_encoding::BASE32.decode(SECRET.as_bytes()).unwrap())
                .unwrap();
        mac.update(&(self.clock.load(Ordering::SeqCst).div_euclid(30) as u64).to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = (digest[19] & 15) as usize;
        let number =
            u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
        format!("{:06}", number % 1_000_000)
    }

    pub(super) fn code_is_current(&self, code: &str) -> bool {
        self.source().verify_current_totp(SECRET, code)
    }

    pub(super) async fn seed_expired_session(&self) -> String {
        self.seed_session(&self.first, "clock_timestamp() - INTERVAL '1 hour'")
            .await
    }

    pub(super) async fn seed_session(&self, name: &LegacyUsername, expires: &str) -> String {
        let token = Uuid::new_v4().to_string();
        let digest = format!("{:x}", Sha256::digest(token.as_bytes()));
        let inserted = within(
            query(&format!(
                "INSERT INTO sessions (token, username, account_id, expires_at)
            SELECT $1, username, account_id, {expires} FROM users WHERE username = $2"
            ))
            .bind(digest)
            .bind(name.as_str())
            .execute(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("synthetic expired device setup failed"));
        assert_eq!(
            inserted.rows_affected(),
            1,
            "fixture did not select its real registered account"
        );
        token
    }

    pub(super) async fn counter(&self, name: &LegacyUsername) -> i64 {
        within(
            query("SELECT otp_last_counter AS counter FROM users WHERE username = $1")
                .bind(name.as_str())
                .fetch_one(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("synthetic account watermark observation failed"))
        .get("counter")
    }

    pub(super) fn next_code(&self) -> String {
        self.clock.fetch_add(30, Ordering::SeqCst);
        self.code()
    }

    pub(super) async fn snapshot(&self) -> Snapshot {
        let mut digests = Vec::new();
        for table in ["users", "sessions", "collaboration_auth_source_schema"] {
            let row = within(
                query(&format!(
                    "SELECT md5(COALESCE(jsonb_agg(to_jsonb(t)
                ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text) AS digest FROM {table} t"
                ))
                .fetch_one(self.pool()),
            )
            .await
            .unwrap_or_else(|_| panic!("private SQL digest snapshot failed"));
            digests.push(row.get("digest"));
        }
        let row = within(
            query(
                "SELECT (SELECT count(*) FROM sessions) AS sessions,
            otp_last_counter AS watermark FROM users WHERE username = $1",
            )
            .bind(self.first.as_str())
            .fetch_one(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("stock/counter snapshot failed"));
        let pins = within(
            query(
                "SELECT oid::bigint AS oid FROM pg_class
            WHERE relnamespace=current_schema()::regnamespace AND relname IN
            ('users','sessions','collaboration_auth_source_schema') ORDER BY relname",
            )
            .fetch_all(self.pool()),
        )
        .await
        .unwrap_or_else(|_| panic!("original source relation observation failed"))
        .into_iter()
        .map(|row| row.get("oid"))
        .collect();
        Snapshot {
            digests,
            pins,
            sessions: row.get("sessions"),
            watermark: row.get("watermark"),
        }
    }

    async fn cleanup(&mut self) {
        self.source.take();
        for pool in &self.pools {
            within(pool.close()).await;
        }
        self.pools.clear();
        if let Some(pool) = &self.pool {
            within(pool.close()).await;
        }
        self.pool.take();
        if let Some(admin) = &self.admin {
            for schema in self.namespaces.iter().rev() {
                within(query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).execute(admin))
                    .await
                    .unwrap_or_else(|_| panic!("owned Session stock namespace cleanup failed"));
            }
            within(admin.close()).await;
        }
        self.admin.take();
    }

    pub(super) async fn checked<T>(case: impl for<'a> FnOnce(&'a mut Self) -> Case<'a, T>) -> T {
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
