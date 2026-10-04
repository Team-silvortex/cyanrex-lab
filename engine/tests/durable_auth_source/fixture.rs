use super::*;
use cyanrex_engine::services::auth_service::durable_source::{
    DurableAccountRef, DurableLogin, DurableRegistration, DurableSession,
};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

pub const PASSWORD: &str = "synthetic-source-password";
pub fn id<T: std::str::FromStr>() -> T
where
    T::Err: std::fmt::Debug,
{
    Uuid::new_v4().to_string().parse().unwrap()
}
pub fn authority() -> AuthorityId {
    "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap()
}
pub fn username() -> LegacyUsername {
    "source-learner".parse().unwrap()
}
pub fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub fn otp(secret: &str) -> String {
    otp_at_counter(secret, Utc::now().timestamp().div_euclid(30))
}
/// A distinct next-step code for a second real authentication within the accepted drift window.
/// Never use this to claim arbitrary repeated logins or to reset a consumed counter.
pub fn next_otp(secret: &str) -> String {
    otp_at_counter(secret, Utc::now().timestamp().div_euclid(30) + 1)
}
fn otp_at_counter(secret: &str, counter: i64) -> String {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(&data_encoding::BASE32.decode(secret.as_bytes()).unwrap())
            .unwrap();
    mac.update(&(counter as u64).to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 15) as usize;
    let number = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", number % 1_000_000)
}

pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub source: DurableAuthSource,
    pub schema: String,
    url: String,
}
impl Fixture {
    pub async fn new() -> Self {
        let url =
            std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test database required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("durable_auth_source_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = Self::pool(&url, &schema).await;
        let source = DurableAuthSource::new(pool.clone(), authority());
        Self {
            admin,
            pool,
            source,
            schema,
            url,
        }
    }
    async fn pool(url: &str, schema: &str) -> PgPool {
        let schema = schema.to_string();
        PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)").bind(schema).execute(connection).await?;
                Ok(())
            })
        }).connect(url).await.unwrap()
    }
    pub async fn ready() -> Self {
        let f = Self::new().await;
        f.source.install_empty_schema().await.unwrap();
        f
    }
    pub async fn reopen(&self) -> DurableAuthSource {
        DurableAuthSource::new(Self::pool(&self.url, &self.schema).await, authority())
    }
    pub async fn login(&self) -> (DurableRegistration, DurableLogin) {
        let created = self.source.register(&username(), PASSWORD).await.unwrap();
        let login = self
            .source
            .login(&username(), PASSWORD, &otp(&created.bootstrap.secret))
            .await
            .unwrap();
        (created, login)
    }
    /// Explicit synthetic Session setup for downstream authorization/rotation tests. Registration
    /// remains real; this is not login evidence and does not alter the account's OTP watermark.
    #[allow(dead_code)] // Shared fixture: only rotation/shape targets need synthetic Session setup.
    pub async fn seed_account_session(&self) -> (DurableRegistration, DurableLogin) {
        let created = self.source.register(&username(), PASSWORD).await.unwrap();
        let login = self.seed_session(&created.account).await;
        (created, login)
    }
    /// Add a test device without consuming another real authentication counter.
    #[allow(dead_code)] // Shared fixture: ordinary source tests retain real login instead.
    pub async fn seed_session(&self, account: &DurableAccountRef) -> DurableLogin {
        let token = Uuid::new_v4().to_string();
        let expires_at = query(
            "INSERT INTO sessions (token, username, account_id, expires_at)
            VALUES ($1, $2, $3, clock_timestamp() + INTERVAL '12 hours') RETURNING expires_at",
        )
        .bind(token_hash(&token))
        .bind(account.username.as_str())
        .bind(account.account_id.as_uuid())
        .fetch_one(&self.pool)
        .await
        .unwrap()
        .get("expires_at");
        DurableLogin {
            token,
            session: DurableSession {
                account: account.clone(),
                expires_at,
            },
        }
    }
    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }
    pub async fn count(&self, table: &str) -> i64 {
        assert!(matches!(table, "users" | "sessions"));
        query(&format!("SELECT count(*) AS count FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("count")
    }
    pub async fn table_count(&self) -> i64 {
        query("SELECT count(*) AS count FROM information_schema.tables WHERE table_schema = $1")
            .bind(&self.schema)
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("count")
    }
    pub async fn wait_blocked(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let waiting: i64 = query("SELECT count(*) AS count FROM pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock'").bind(&self.schema).fetch_one(&self.admin).await.unwrap().get("count");
                if waiting > 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("source operation did not reach the synthetic lock");
    }
    pub async fn drain(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut held = Vec::new();
            for _ in 0..4 {
                let mut connection = self.pool.acquire().await.unwrap();
                query("SELECT 1").execute(&mut *connection).await.unwrap();
                held.push(connection);
            }
        })
        .await
        .unwrap();
    }
    pub async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
