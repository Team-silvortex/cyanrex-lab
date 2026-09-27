use super::*;
use sqlx_postgres::PgPool;
use uuid::Uuid;

pub fn id<T: std::str::FromStr>() -> T
where
    T::Err: std::fmt::Debug,
{
    Uuid::new_v4().to_string().parse().unwrap()
}

pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap(),
        workspace_id: "51f0325b-ea1b-42f9-9b39-71d063b8d327".parse().unwrap(),
    }
}

pub fn username() -> LegacyUsername {
    "synthetic-student".parse().unwrap()
}
pub fn account() -> LegacyAccountId {
    "bdc46637-1a2a-4cd9-b617-915328788824".parse().unwrap()
}
pub fn principal() -> PrincipalId {
    "d7ed9514-330f-4cd7-9a30-32741ee8d0cf".parse().unwrap()
}

pub struct Fixture {
    pub admin: PgPool,
    pub pool: PgPool,
    pub store: CollaborationIdentityStore,
    pub schema: String,
    url: String,
}

impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("disposable test database URL required");
        let admin = PgPoolOptions::new().connect(&url).await.unwrap();
        let schema = format!("collaboration_identity_{}", Uuid::new_v4().simple());
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = Self::pool(&url, &schema).await;
        let store = CollaborationIdentityStore::new(pool.clone());
        Self {
            admin,
            pool,
            store,
            schema,
            url,
        }
    }

    async fn pool(url: &str, schema: &str) -> PgPool {
        let schema = schema.to_string();
        PgPoolOptions::new().max_connections(4).after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                query("SELECT set_config('search_path', $1, false), set_config('application_name', $1, false)")
                    .bind(schema).execute(connection).await?;
                Ok(())
            })
        }).connect(url).await.unwrap()
    }

    pub async fn ready() -> Self {
        let f = Self::new().await;
        f.store.install_schema().await.unwrap();
        f.store.provision_legacy_workspace(scope()).await.unwrap();
        f
    }

    pub async fn reopen(&self) -> CollaborationIdentityStore {
        CollaborationIdentityStore::new(Self::pool(&self.url, &self.schema).await)
    }

    pub async fn bind(
        &self,
    ) -> cyanrex_engine::services::collaboration_identity_store::StoredLegacyIdentity {
        self.store
            .bind_legacy_account(scope(), &username(), account())
            .await
            .unwrap()
    }

    pub async fn sql(&self, statement: &str) {
        query(statement).execute(&self.pool).await.unwrap();
    }

    pub async fn count(&self, table: &str) -> i64 {
        // Only fixed test table names, never application input.
        assert!(
            table.starts_with("collaboration_")
                && table.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        );
        query(&format!("SELECT count(*) AS count FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .get("count")
    }

    pub async fn wait_blocked(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let waiting: i64 = query("SELECT count(*) AS count FROM pg_stat_activity WHERE application_name = $1 AND wait_event_type = 'Lock'")
                    .bind(&self.schema).fetch_one(&self.admin).await.unwrap().get("count");
                if waiting > 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("synthetic identity mutation did not reach the lock");
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
        .expect("cancelled identity transaction did not release its connection");
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
