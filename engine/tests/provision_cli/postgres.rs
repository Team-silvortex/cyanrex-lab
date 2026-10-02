use super::*;
use local::{success, Local};
use serde_json::{json, Value};
use sqlx_core::transaction::Transaction;
use std::{fs, process::Child, time::Duration};

pub struct ProvisionFixture {
    pub db: source_fixture::Fixture,
    pub files: Local,
    _guard: Transaction<'static, sqlx_postgres::Postgres>,
}
impl ProvisionFixture {
    pub async fn new() -> Self {
        // Coordinate with C1-I's database-global event-hook fixtures, not production work.
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("CYANREX_TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let mut guard = pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.test.bootstrap.event-hooks'))")
            .execute(&mut *guard).await.unwrap();
        let db = source_fixture::Fixture::new().await;
        let files = Local::new();
        let options = db.pool.connect_options();
        let mut target = Local::target();
        let socket = options
            .get_socket()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| {
                options
                    .get_host()
                    .starts_with('/')
                    .then(|| options.get_host().to_owned())
            });
        target["transport"] = if let Some(directory) = socket {
            json!({"kind":"unix", "directory":directory})
        } else {
            json!({"kind":"loopback", "address":options.get_host()})
        };
        target["port"] = json!(options.get_port());
        target["database"] = json!(options.get_database().unwrap());
        target["database_user"] = json!(options.get_username());
        target["database_password"] = json!(std::env::var("CYANREX_TEST_DATABASE_PASSWORD")
            .expect("explicit disposable DB password, possibly empty"));
        target["schema"] = json!(db.schema);
        files.set_config(&target);
        Self {
            db,
            files,
            _guard: guard,
        }
    }
    pub fn plan(&self) -> String {
        let report = success(&self.files.run("plan"));
        assert_eq!(report["namespace_state"], "empty");
        assert_eq!(report["target"]["schema"], self.db.schema);
        assert_eq!(report["initial_policy"]["deployment_granted"], true);
        report["confirmation"].as_str().unwrap().to_owned()
    }
    pub fn enrollment(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.files.enrollment).unwrap()).unwrap()
    }
    pub async fn blocked(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: i64 = query("SELECT count(*) AS n FROM pg_stat_activity WHERE application_name='cyanrex-provision' AND wait_event_type='Lock'")
                    .fetch_one(&self.db.admin).await.unwrap().get("n");
                if waiting > 0 && self.files.enrollment.exists() { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("CLI did not reach the installer fence");
    }
    pub async fn fence(&self) -> Transaction<'static, sqlx_postgres::Postgres> {
        let mut tx = self.db.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.auth-source.' || current_schema()))")
            .execute(&mut *tx).await.unwrap();
        tx
    }
    pub fn spawn(&self, confirmation: &str) -> Child {
        self.files
            .apply(confirmation)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    }
    pub async fn cleanup(self) {
        self.db.cleanup().await;
    }
}
