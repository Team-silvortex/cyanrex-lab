use super::{
    config::{Config, Transport},
    Result,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx_core::{
    connection::{ConnectOptions, Connection},
    error::Error,
    query::query,
    row::Row,
};
use sqlx_postgres::{PgConnectOptions, PgConnection, PgPool, PgPoolOptions, PgSslMode};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

pub struct Database {
    pub pool: PgPool,
    expected: Arc<OnceLock<String>>,
}
pub struct Snapshot {
    identity: Value,
    relations: i64,
    functions: i64,
    types: i64,
    pub can_create: bool,
}
impl Snapshot {
    pub fn empty(&self) -> bool {
        self.relations == 0 && self.functions == 0 && self.types == 0
    }
    fn digest(&self, target: &Value) -> String {
        let intent = json!({"operation":"cyanrex-empty-authority-v1", "target":target,
            "physical_target":self.identity,"initial_policy":initial_policy()});
        format!("{:x}", Sha256::digest(intent.to_string().as_bytes()))
    }
    pub fn confirmation(&self, config: &Config) -> String {
        self.digest(&config.public_target())
    }
    pub fn report(&self, config: &Config, plan: bool) -> Value {
        let mut report = json!({"format_version":1,"target":config.public_target(),"physical_target":self.identity,
            "namespace_state":if self.empty() {"empty"} else {"occupied"},
            "objects":{"relations":self.relations,"functions":self.functions,"types":self.types},
            "can_create":self.can_create, "initial_policy":initial_policy(),"live_runtime_changed":false,
            "observation":"read-only catalog snapshot, not a health check, bootstrap receipt, reservation or secret-delivery proof"});
        if plan {
            report["confirmation"] = json!(self.confirmation(config));
        }
        report
    }
}
fn initial_policy() -> Value {
    json!({"membership":"active","roles":["cyanrex.teaching.teacher","cyanrex.workspace.owner"],
        "deployment_granted":true,"audit_baselines":true,"session_issued":false})
}
impl Database {
    pub async fn connect(config: &Config) -> Result<Self> {
        let expected = Arc::new(OnceLock::<String>::new());
        let pinned = expected.clone();
        let target = config.public_target();
        let mut options = PgConnectOptions::new_without_pgpass()
            .host("localhost")
            .port(config.port)
            .database(&config.database)
            .username(&config.database_user)
            .password(&config.database_password)
            .ssl_mode(PgSslMode::Disable)
            .application_name("cyanrex-provision")
            .disable_statement_logging()
            .options([
                ("search_path", config.schema.as_str()),
                ("statement_timeout", "5000"),
                ("lock_timeout", "2000"),
                ("idle_in_transaction_session_timeout", "10000"),
            ]);
        options = match &config.transport {
            Transport::Unix { directory } => options.socket(directory),
            Transport::Loopback { address } => options.host(address),
        };
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(None)
            .max_lifetime(None)
            .after_connect(move |connection, _| {
                let target = target.clone();
                let pinned = pinned.clone();
                Box::pin(async move {
                    // Every replacement connection rechecks the reviewed physical identity. A
                    // changed socket/forwarded database cannot silently receive a pending write.
                    if let Some(expected) = pinned.get() {
                        let snapshot = inspect(connection, &target).await?;
                        if &snapshot.digest(&target) != expected {
                            return Err(Error::Protocol("provision target changed".into()));
                        }
                    }
                    Ok(())
                })
            })
            .connect_with(options)
            .await
            .map_err(|_| "storage_unavailable")?;
        Ok(Self { pool, expected })
    }
    pub fn pin(&self, confirmation: String) -> Result<()> {
        self.expected
            .set(confirmation)
            .map_err(|_| "confirmation_mismatch")
    }
    pub async fn snapshot(&self, config: &Config) -> Result<Snapshot> {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut connection = self.pool.acquire().await?;
            inspect(&mut connection, &config.public_target()).await
        })
        .await
        .map_err(|_| "storage_unavailable")?
        .map_err(|_| "target_unavailable")
    }
}
async fn inspect(
    connection: &mut PgConnection,
    target: &Value,
) -> std::result::Result<Snapshot, Error> {
    let mut tx = connection.begin().await?;
    query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let row = query("SELECT pg_catalog.current_database() AS database, current_user::TEXT AS database_user,
        n.nspname::TEXT AS schema, n.oid::BIGINT AS namespace_oid, d.oid::BIGINT AS database_oid,
        pg_catalog.pg_get_userbyid(n.nspowner)::TEXT AS namespace_owner,
        (SELECT system_identifier::TEXT FROM pg_catalog.pg_control_system()) AS system_identifier,
        pg_catalog.current_schemas(false) = ARRAY[n.nspname]::name[] AND pg_catalog.pg_my_temp_schema() = 0 AS pinned,
        pg_catalog.has_schema_privilege(n.oid, 'CREATE') AND pg_catalog.has_schema_privilege(n.oid, 'USAGE') AS can_create,
        (SELECT count(*) FROM pg_catalog.pg_class WHERE relnamespace=n.oid) AS relations,
        (SELECT count(*) FROM pg_catalog.pg_proc WHERE pronamespace=n.oid) AS functions,
        (SELECT count(*) FROM pg_catalog.pg_type WHERE typnamespace=n.oid) AS types
        FROM pg_catalog.pg_namespace n, pg_catalog.pg_database d
        WHERE n.nspname=pg_catalog.current_schema() AND d.datname=pg_catalog.current_database()")
        .fetch_one(&mut *tx).await?;
    for field in ["database", "database_user", "schema"] {
        if target[field].as_str() != Some(row.try_get::<&str, _>(field)?) {
            return Err(Error::Protocol("provision target mismatch".into()));
        }
    }
    if !row.try_get::<bool, _>("pinned")? {
        return Err(Error::Protocol("ambiguous provision namespace".into()));
    }
    let snapshot = Snapshot {
        identity: json!({"system_identifier":row.try_get::<String,_>("system_identifier")?,
            "database_oid":row.try_get::<i64,_>("database_oid")?,"namespace_oid":row.try_get::<i64,_>("namespace_oid")?,
            "namespace_owner":row.try_get::<String,_>("namespace_owner")?}),
        relations: row.try_get("relations")?,
        functions: row.try_get("functions")?,
        types: row.try_get("types")?,
        can_create: row.try_get("can_create")?,
    };
    tx.commit().await?;
    Ok(snapshot)
}
