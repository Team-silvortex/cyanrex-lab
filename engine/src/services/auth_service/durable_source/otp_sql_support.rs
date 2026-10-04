use super::*;
use crate::sqlx_compat::{query, PgPool, PgPoolOptions, Row};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub(super) const PASSWORD: &str = "synthetic-otp-password";
pub(super) const NEW_PASSWORD: &str = "synthetic-otp-replacement";
pub(super) const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const INITIAL_TIME: i64 = 1_800_000_000;

pub(super) fn username() -> crate::models::collaboration::LegacyUsername {
    "otp-freshness-learner".parse().unwrap()
}

pub(super) struct Fixture {
    pub(super) admin: PgPool,
    pub(super) pool: PgPool,
    pub(super) source: DurableAuthSource,
    pub(super) schema: String,
    pub(super) clock: Arc<AtomicI64>,
}

impl Fixture {
    pub(super) async fn ready() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("requires a disposable CYANREX_TEST_DATABASE_URL");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("durable_otp_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let namespace = schema.clone();
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .after_connect(move |connection, _| {
                let namespace = namespace.clone();
                Box::pin(async move {
                    query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                        .bind(namespace).execute(connection).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        let clock = Arc::new(AtomicI64::new(INITIAL_TIME));
        let mut source =
            DurableAuthSource::new(pool.clone(), Uuid::new_v4().to_string().parse().unwrap());
        let now = clock.clone();
        source.otp_clock = Some(Arc::new(move || {
            chrono::DateTime::from_timestamp(now.load(Ordering::SeqCst), 0).unwrap()
        }));
        source.install_empty_schema().await.unwrap();
        source.register(&username(), PASSWORD).await.unwrap();
        // A fixed synthetic key makes the old/new-window noncollision assertion deterministic.
        query("UPDATE users SET totp_secret = $1 WHERE username = $2")
            .bind(SECRET)
            .bind(username().as_str())
            .execute(&pool)
            .await
            .unwrap();
        Self {
            admin,
            pool,
            source,
            schema,
            clock,
        }
    }

    pub(super) fn code(&self) -> String {
        let secret = data_encoding::BASE32.decode(SECRET.as_bytes()).unwrap();
        let mut mac = Hmac::<Sha1>::new_from_slice(&secret).unwrap();
        mac.update(&(self.clock.load(Ordering::SeqCst).div_euclid(30) as u64).to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = (digest[19] & 15) as usize;
        let number =
            u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
        format!("{:06}", number % 1_000_000)
    }

    pub(super) fn next_code(&self) -> String {
        self.clock.fetch_add(30, Ordering::SeqCst);
        self.code()
    }

    pub(super) fn expire(&self, code: &str) {
        assert!(self.source.verify_current_totp(SECRET, code));
        self.clock.fetch_add(90, Ordering::SeqCst);
        assert!(!self.source.verify_current_totp(SECRET, code));
    }

    pub(super) async fn snapshot(&self) -> Vec<String> {
        let mut values = Vec::new();
        for table in ["users", "sessions", "collaboration_auth_source_schema"] {
            values.push(query(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text AS state FROM {table} t"))
                .fetch_one(&self.pool).await.unwrap().get("state"));
        }
        values
    }

    pub(super) async fn count_sessions(&self) -> i64 {
        query("SELECT count(*) AS count FROM sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("count")
    }

    pub(super) async fn trigger(&self, timing: &str) {
        assert!(matches!(timing, "AFTER INSERT" | "AFTER DELETE"));
        query("CREATE SEQUENCE otp_fault_hits")
            .execute(&self.pool)
            .await
            .unwrap();
        query(
            "CREATE FUNCTION otp_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('otp_fault_hits');
            PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 76531);
            RETURN NULL; END $$",
        )
        .execute(&self.pool)
        .await
        .unwrap();
        query(&format!("CREATE TRIGGER otp_fault {timing} ON sessions FOR EACH ROW EXECUTE FUNCTION otp_fault()"))
            .execute(&self.pool).await.unwrap();
    }

    pub(super) async fn trigger_calls(&self) -> i64 {
        query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS calls FROM otp_fault_hits")
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("calls")
    }

    pub(super) fn independent_source(&self, pool: PgPool) -> DurableAuthSource {
        let mut source = DurableAuthSource::new(pool, self.source.authority_id);
        let clock = self.clock.clone();
        source.otp_clock = Some(Arc::new(move || {
            chrono::DateTime::from_timestamp(clock.load(Ordering::SeqCst), 0).unwrap()
        }));
        source
    }

    pub(super) async fn separate_pool(&self, application: &str) -> PgPool {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let schema = self.schema.clone();
        let application = application.to_string();
        PgPoolOptions::new().max_connections(1).after_connect(move |connection, _| {
            let schema = schema.clone();
            let application = application.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $2, false)")
                    .bind(schema).bind(application).execute(connection).await?;
                Ok(())
            })
        }).connect(&url).await.unwrap()
    }

    pub(super) async fn wait_blocked(&self, blocker_pid: i32) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let blocked: bool = query("SELECT EXISTS (SELECT 1 FROM pg_stat_activity a WHERE application_name = $1 AND $2 = ANY(pg_blocking_pids(a.pid))) AS blocked")
                    .bind(&self.schema).bind(blocker_pid).fetch_one(&self.admin).await.unwrap().get("blocked");
                if blocked { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("operation did not reach the exact synthetic blocker");
    }

    pub(super) async fn cleanup(self) {
        self.pool.close().await;
        query(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
