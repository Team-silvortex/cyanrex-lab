use super::config::scope;
use cyanrex_engine::{
    platform_host::{
        inspect_platform_database, PlatformCatalogObservation, PlatformDatabaseTarget,
        PlatformHostPreflightConfig, PlatformHostPreflightError, PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use futures_util::FutureExt;
use sqlx_core::{connection::ConnectOptions, query::query, row::Row};
use sqlx_postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};
use uuid::Uuid;

type Case<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";

pub(super) async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .unwrap_or_else(|_| panic!("bounded platform catalog fixture operation timed out"))
}

// Parse only the explicitly supplied URL. In particular, neither PG* nor Engine defaults
// may supply a missing account, database, password or transport coordinate.
fn admin_options() -> PgConnectOptions {
    let raw = std::env::var("CYANREX_TEST_DATABASE_URL")
        .expect("set an explicit disposable CYANREX_TEST_DATABASE_URL");
    let url = reqwest::Url::parse(&raw)
        .unwrap_or_else(|_| panic!("invalid explicit disposable database URL"));
    assert!(matches!(url.scheme(), "postgres" | "postgresql"));
    assert!(
        !url.username().is_empty(),
        "explicit database role required"
    );
    let database = url.path().strip_prefix('/').unwrap_or("");
    assert!(
        !database.is_empty() && !database.contains('/'),
        "explicit database required"
    );
    let pairs: Vec<_> = url.query_pairs().collect();
    let host = pairs
        .iter()
        .find(|(name, _)| name == "host")
        .map(|(_, value)| value.as_ref())
        .or_else(|| url.host_str())
        .expect("explicit database transport required");
    let port = pairs
        .iter()
        .find(|(name, _)| name == "port")
        .map(|(_, value)| {
            value
                .parse::<u16>()
                .expect("invalid explicit database port")
        })
        .or(url.port())
        .unwrap_or(5432);
    let options = PgConnectOptions::new_without_pgpass()
        .username(url.username())
        .password(url.password().unwrap_or(""))
        .database(database)
        .host(host)
        .port(port)
        .ssl_mode(PgSslMode::Disable)
        .application_name("cyanrex-platform-catalog-fixture")
        .disable_statement_logging()
        .options([("statement_timeout", "10000"), ("lock_timeout", "2000")]);
    if host.starts_with('/') {
        options.socket(host)
    } else {
        options
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Snapshot {
    digest: String,
    pub(super) tables: i64,
}

pub(super) struct Fixture {
    pub(super) database: String,
    pub(super) role: String,
    pub(super) source: String,
    pub(super) task: String,
    pub(super) artifacts: String,
    base: PgPool,
    owner: Option<PgPool>,
    runtime: Option<PgPool>,
    database_created: bool,
    role_created: bool,
    auxiliary_roles: Vec<String>,
    config: Option<PlatformHostPreflightConfig>,
}

impl Fixture {
    async fn empty() -> Self {
        let base = within(
            PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(10))
                .connect_with(admin_options()),
        )
        .await
        .unwrap_or_else(|_| panic!("explicit disposable database administrator unavailable"));
        let suffix = Uuid::new_v4().simple().to_string();
        Self {
            database: format!("catalog_db_{suffix}"),
            role: format!("catalog_role_{suffix}"),
            source: "source_private".into(),
            task: "task_private".into(),
            artifacts: "artifacts_private".into(),
            base,
            owner: None,
            runtime: None,
            database_created: false,
            role_created: false,
            config: None,
            auxiliary_roles: Vec::new(),
        }
    }

    async fn prepare(&mut self) {
        self.base_sql(&format!("CREATE ROLE {} LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '{PASSWORD}'", self.role)).await;
        self.role_created = true;
        self.base_sql(&format!("CREATE DATABASE {}", self.database))
            .await;
        self.database_created = true;
        let options = self
            .base
            .connect_options()
            .as_ref()
            .clone()
            .database(&self.database);
        self.owner = Some(
            within(
                PgPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(10))
                    .connect_with(options.clone()),
            )
            .await
            .unwrap_or_else(|_| panic!("owned catalog database administrator unavailable")),
        );
        for statement in [
            format!(
                "REVOKE ALL PRIVILEGES ON DATABASE {} FROM PUBLIC",
                self.database
            ),
            format!(
                "GRANT CONNECT ON DATABASE {} TO {}",
                self.database, self.role
            ),
            format!(
                "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only = on",
                self.role, self.database
            ),
            "REVOKE EXECUTE ON FUNCTION pg_catalog.pg_control_system() FROM PUBLIC".into(),
            format!(
                "GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {}",
                self.role
            ),
        ] {
            self.sql(&statement).await;
        }
        for schema in [&self.source, &self.task, &self.artifacts] {
            self.sql(&format!("CREATE SCHEMA {schema}")).await;
            self.sql(&format!("REVOKE ALL ON SCHEMA {schema} FROM PUBLIC"))
                .await;
            self.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", self.role))
                .await;
        }
        let row = within(query("SELECT system_identifier::text AS system_id, (SELECT oid::bigint FROM pg_catalog.pg_database WHERE datname = current_database()) AS database_oid FROM pg_catalog.pg_control_system()")
            .fetch_one(self.owner())).await
            .unwrap_or_else(|_| panic!("owned database physical identity unavailable"));
        let system_id = row.get::<String, _>("system_id").parse::<u64>().unwrap();
        let database_oid = u32::try_from(row.get::<i64, _>("database_oid")).unwrap();
        self.config = Some(
            PlatformHostPreflightConfig::new(
                "https://platform.example",
                scope(),
                PlatformNamespaces::new(&self.source, &self.task, &self.artifacts).unwrap(),
                PlatformDatabaseTarget::new(&self.database, &self.role, system_id, database_oid)
                    .unwrap(),
                SessionStockLimits::new(2, 8).unwrap(),
            )
            .unwrap(),
        );
        let source = self.source.clone();
        self.runtime = Some(
            within(
                PgPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(10))
                    .after_connect(move |connection, _| {
                        let source = source.clone();
                        Box::pin(async move {
                            query("SELECT pg_catalog.set_config('search_path', $1, false)")
                                .bind(source)
                                .execute(connection)
                                .await?;
                            Ok(())
                        })
                    })
                    .connect_with(options.username(&self.role).password(PASSWORD)),
            )
            .await
            .unwrap_or_else(|_| panic!("owned read-only runtime role unavailable")),
        );
    }

    pub(super) fn config(&self) -> &PlatformHostPreflightConfig {
        self.config.as_ref().unwrap()
    }
    pub(super) fn runtime(&self) -> &PgPool {
        self.runtime.as_ref().unwrap()
    }
    pub(super) fn owner(&self) -> &PgPool {
        self.owner.as_ref().unwrap()
    }

    pub(super) async fn base_sql(&self, statement: &str) {
        within(query(statement).execute(&self.base))
            .await
            .unwrap_or_else(|_| panic!("owned database or role setup failed"));
    }

    pub(super) async fn sql(&self, statement: &str) {
        within(query(statement).execute(self.owner()))
            .await
            .unwrap_or_else(|_| panic!("owned catalog setup failed"));
    }

    pub(super) async fn snapshot(&self) -> Snapshot {
        let row = within(query("SELECT md5(concat_ws('|',
            (SELECT coalesce(string_agg(oid::text || ':' || nspname || ':' || nspowner::text || ':' || coalesce(nspacl::text, ''), '|' ORDER BY oid), '') FROM pg_catalog.pg_namespace WHERE nspname = ANY($1)),
            (SELECT coalesce(string_agg(c.oid::text || ':' || c.relname || ':' || c.relkind::text, '|' ORDER BY c.oid), '') FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = ANY($1)),
            (SELECT datdba::text || ':' || coalesce(datacl::text, '') FROM pg_catalog.pg_database WHERE datname = current_database()),
            (SELECT proacl::text FROM pg_catalog.pg_proc WHERE oid = 'pg_catalog.pg_control_system()'::regprocedure),
            (SELECT oid::text || ':' || rolsuper::text || ':' || rolinherit::text || ':' || rolcreaterole::text || ':' || rolcreatedb::text || ':' || rolcanlogin::text || ':' || rolreplication::text || ':' || rolbypassrls::text FROM pg_catalog.pg_roles WHERE rolname = $2),
            (SELECT coalesce(string_agg(roleid::text || ':' || member::text || ':' || admin_option::text, '|' ORDER BY roleid, member), '') FROM pg_catalog.pg_auth_members WHERE member = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = $2)),
            (SELECT coalesce(string_agg(p.oid::text, '|' ORDER BY p.oid), '') FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = ANY($1)),
            (SELECT coalesce(string_agg(t.oid::text, '|' ORDER BY t.oid), '') FROM pg_catalog.pg_type t JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace WHERE n.nspname = ANY($1))
        )) AS digest,
        (SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = ANY($1)) AS tables")
            .bind(vec![self.source.clone(), self.task.clone(), self.artifacts.clone()])
            .bind(&self.role)
            .fetch_one(self.owner())).await
            .unwrap_or_else(|_| panic!("independent catalog snapshot failed"));
        Snapshot {
            digest: row.get("digest"),
            tables: row.get("tables"),
        }
    }

    pub(super) async fn observe(
        &self,
    ) -> Result<PlatformCatalogObservation, PlatformHostPreflightError> {
        let mut connection = within(self.runtime().acquire())
            .await
            .unwrap_or_else(|_| panic!("caller-owned catalog connection unavailable"));
        within(inspect_platform_database(self.config(), &mut connection)).await
    }

    pub(super) async fn inspect_unchanged(&self) -> (Result<(), PlatformHostPreflightError>, bool) {
        let before = self.snapshot().await;
        let result = self.observe().await.map(|_| ());
        let after = self.snapshot().await;
        (result, before == after)
    }

    pub(super) async fn additional_role(&mut self) -> String {
        let name = format!("catalog_aux_{}", Uuid::new_v4().simple());
        self.base_sql(&format!("CREATE ROLE {name} NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS")).await;
        self.auxiliary_roles.push(name.clone());
        name
    }

    pub(super) async fn owner_name(&self) -> String {
        within(query("SELECT current_user::text AS name").fetch_one(self.owner()))
            .await
            .unwrap_or_else(|_| panic!("owned database administrator identity unavailable"))
            .get("name")
    }

    pub(super) async fn runtime_is_read_only_and_unprivileged(&self) -> bool {
        let row = within(query("SELECT current_user = $1 AND session_user = $1 AS exact_role,
            current_setting('transaction_read_only') = 'on' AS readonly,
            current_setting('search_path') = $2 AS exact_path,
            NOT has_database_privilege(current_user, current_database(), 'CREATE')
                AND NOT has_database_privilege(current_user, current_database(), 'TEMP') AS safe_database,
            has_function_privilege(current_user, 'pg_catalog.pg_control_system()', 'EXECUTE') AS control,
            (SELECT bool_and(has_schema_privilege(current_user, n.oid, 'USAGE')
                AND NOT has_schema_privilege(current_user, n.oid, 'CREATE')
                AND n.nspowner <> (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = current_user))
                FROM pg_catalog.pg_namespace n WHERE n.nspname = ANY($3)) AS schemas")
            .bind(&self.role).bind(&self.source)
            .bind(vec![self.source.clone(), self.task.clone(), self.artifacts.clone()])
            .fetch_one(self.runtime())).await
            .unwrap_or_else(|_| panic!("runtime catalog privilege oracle failed"));
        [
            "exact_role",
            "readonly",
            "exact_path",
            "safe_database",
            "control",
            "schemas",
        ]
        .into_iter()
        .all(|column| row.get::<bool, _>(column))
    }

    async fn cleanup(&mut self) {
        self.config.take();
        if let Some(pool) = self.runtime.take() {
            within(pool.close()).await;
        }
        if let Some(pool) = self.owner.take() {
            within(pool.close()).await;
        }
        let database_removed = if self.database_created {
            within(query(&format!("DROP DATABASE {}", self.database)).execute(&self.base))
                .await
                .is_ok()
        } else {
            true
        };
        let role_removed = if self.role_created {
            within(query(&format!("DROP ROLE {}", self.role)).execute(&self.base))
                .await
                .is_ok()
        } else {
            true
        };
        let mut auxiliary_removed = true;
        for role in self.auxiliary_roles.iter().rev() {
            auxiliary_removed &= within(query(&format!("DROP ROLE {role}")).execute(&self.base))
                .await
                .is_ok();
        }
        within(self.base.close()).await;
        assert!(database_removed, "owned catalog database cleanup failed");
        assert!(role_removed, "owned catalog role cleanup failed");
        assert!(
            auxiliary_removed,
            "owned auxiliary catalog role cleanup failed"
        );
    }

    pub(super) async fn checked<T>(case: impl for<'a> FnOnce(&'a mut Self) -> Case<'a, T>) -> T {
        let mut fixture = Self::empty().await;
        let outcome = AssertUnwindSafe(async {
            fixture.prepare().await;
            case(&mut fixture).await
        })
        .catch_unwind()
        .await;
        let cleanup = AssertUnwindSafe(fixture.cleanup()).catch_unwind().await;
        if let Err(panic) = cleanup {
            std::panic::resume_unwind(panic);
        }
        match outcome {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}
