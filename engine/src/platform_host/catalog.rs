use super::{PlatformHostPreflightConfig, PlatformHostPreflightError, Result};
use crate::sqlx_compat::{self as sqlx, Postgres, Row};
use sqlx_core::{connection::Connection, executor::Executor, transaction::Transaction};
use sqlx_postgres::PgConnection;
use std::time::Duration;

mod expectation;
pub use expectation::{inspect_platform_database_expected, PlatformCatalogExpectation};

#[derive(Clone, Copy, PartialEq, Eq)]
struct NamespaceIdentity {
    oid: u32,
    owner_oid: u32,
}

/// Opaque read-time catalog observation, not a runtime handle, authorization or health receipt.
#[derive(PartialEq, Eq)]
pub struct PlatformCatalogObservation {
    config: PlatformHostPreflightConfig,
    role_oid: u32,
    database_owner_oid: u32,
    namespaces: [NamespaceIdentity; 3],
}

impl PlatformCatalogObservation {
    pub(super) fn config_matches(&self, config: &PlatformHostPreflightConfig) -> bool {
        &self.config == config
    }

    pub(super) fn namespace_oids(&self) -> [u32; 3] {
        self.namespaces.map(|namespace| namespace.oid)
    }

    /// Equality compares observations only; it does not establish continuing catalog identity.
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

/// Borrow an idle caller-owned connection for one new catalog-only transaction.
/// SQLx-managed outer transactions are rejected without SQL. A single SAVEPOINT/RELEASE batch
/// also rejects raw BEGIN without committing, rolling back or changing caller settings. That
/// batch briefly adds and removes one savepoint; it performs no business read or mutation.
/// Once an owned SQLx transaction object is acquired, timeout/cancellation drops it and queues
/// rollback for later driver work. An interrupted BEGIN may reach the server before that object
/// is returned. Neither this nor interrupted boundary detection proves the server's final state.
/// No storage is installed or implicitly connected.
pub async fn inspect_platform_database(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
) -> Result<PlatformCatalogObservation> {
    if connection.is_in_transaction() {
        return Err(PlatformHostPreflightError::InvalidConfig);
    }
    // The deadline includes the idle probe, BEGIN, settings/catalog reads and COMMIT/ROLLBACK.
    // Dropping a timed-out future retains SQLx's normal queued transaction rollback semantics.
    tokio::time::timeout(Duration::from_secs(10), async {
        require_idle(connection).await?;
        inspect_owned(config, connection).await
    })
    .await
    .map_err(|_| PlatformHostPreflightError::StorageUnavailable)?
}

pub(super) async fn require_idle(connection: &mut PgConnection) -> Result<()> {
    // SQLx's transaction depth does not track caller-issued raw BEGIN. Do not infer idle from a
    // NULL transaction ID, timestamp comparison or a transaction setting. A simple-query batch
    // executes these two control statements together: an outer transaction restores its previous
    // savepoint stack, while an idle backend rejects SAVEPOINT with 25P01 before either can run.
    // Even a same-named caller savepoint survives because RELEASE removes only the newest one.
    let probe = Executor::execute(&mut *connection,
        "SAVEPOINT cyanrex_platform_preflight_probe; RELEASE SAVEPOINT cyanrex_platform_preflight_probe").await;
    match probe {
        Ok(_) => Err(PlatformHostPreflightError::InvalidConfig),
        Err(error) => match error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref()
        {
            Some("25P01") => Ok(()),
            Some("25P02") => Err(PlatformHostPreflightError::InvalidConfig),
            _ => Err(PlatformHostPreflightError::StorageUnavailable),
        },
    }
}

/// Re-observe an original target; never adopt a replacement or establish runtime permission.
pub async fn verify_platform_database(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected: &PlatformCatalogObservation,
) -> Result<()> {
    if config != &expected.config {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    let current = inspect_platform_database(config, connection).await?;
    if !expected.matches(&current) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    Ok(())
}

async fn inspect_owned(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
) -> Result<PlatformCatalogObservation> {
    let mut tx = connection
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?;
    let observed = collect(config, &mut tx).await;
    match observed {
        Ok(observation) => {
            tx.commit().await?;
            Ok(observation)
        }
        Err(error) => {
            // Still inside the same overall deadline; never wait unboundedly for rollback.
            tx.rollback().await?;
            Err(error)
        }
    }
}

pub(super) async fn collect(
    config: &PlatformHostPreflightConfig,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<PlatformCatalogObservation> {
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut **tx)
        .await?;
    let row = sqlx::query(
        "SELECT pg_catalog.current_database()::text AS database,
        current_user::text AS current_role, session_user::text AS session_role,
        r.oid::bigint AS role_oid, r.rolsuper, r.rolbypassrls, r.rolcreaterole,
        r.rolcreatedb, r.rolreplication, r.rolcanlogin,
        d.oid::bigint AS database_oid, d.datdba::bigint AS database_owner_oid, d.datallowconn,
        pg_catalog.has_database_privilege(r.oid, d.oid, 'CONNECT') AS can_connect,
        pg_catalog.has_database_privilege(r.oid, d.oid, 'CREATE') AS can_create,
        pg_catalog.has_database_privilege(r.oid, d.oid, 'TEMP') AS can_temp,
        EXISTS (SELECT 1 FROM pg_catalog.pg_auth_members m WHERE m.member = r.oid) AS membership,
        pg_catalog.current_schemas(false) = ARRAY[$1]::name[] AS source_only,
        pg_catalog.pg_my_temp_schema() = 0 AS no_temp
        FROM pg_catalog.pg_roles r JOIN pg_catalog.pg_database d
        ON d.datname = pg_catalog.current_database() WHERE r.rolname = current_user",
    )
    .bind(config.namespaces().source())
    .fetch_one(&mut **tx)
    .await?;
    let current_role = row.try_get::<String, _>("current_role")?;
    let session_role = row.try_get::<String, _>("session_role")?;
    if current_role != session_role {
        return Err(PlatformHostPreflightError::UnsafeRole);
    }
    let target = config.target();
    let database_oid = oid(row.try_get("database_oid")?)?;
    if row.try_get::<String, _>("database")? != target.database()
        || database_oid != target.database_oid()
        || session_role != target.role()
    {
        return Err(PlatformHostPreflightError::TargetMismatch);
    }
    let role_oid = oid(row.try_get("role_oid")?)?;
    let database_owner_oid = oid(row.try_get("database_owner_oid")?)?;
    for flag in [
        "rolsuper",
        "rolbypassrls",
        "rolcreaterole",
        "rolcreatedb",
        "rolreplication",
        "can_create",
        "can_temp",
        "membership",
    ] {
        if row.try_get::<bool, _>(flag)? {
            return Err(PlatformHostPreflightError::UnsafeRole);
        }
    }
    if !row.try_get::<bool, _>("rolcanlogin")?
        || !row.try_get::<bool, _>("can_connect")?
        || !row.try_get::<bool, _>("datallowconn")?
        || role_oid == database_owner_oid
    {
        return Err(PlatformHostPreflightError::UnsafeRole);
    }
    if !row.try_get::<bool, _>("source_only")? || !row.try_get::<bool, _>("no_temp")? {
        return Err(PlatformHostPreflightError::UnsafeNamespace);
    }
    // The role must have explicit permission for this read-only control function. Failure has
    // no fallback to weaker address/name identity and never grants or changes permissions.
    let control = sqlx::query(
        "SELECT system_identifier::text AS system_identifier
        FROM pg_catalog.pg_control_system()",
    )
    .fetch_one(&mut **tx)
    .await?;
    let system_identifier = control
        .try_get::<String, _>("system_identifier")?
        .parse::<u64>()
        .map_err(|_| PlatformHostPreflightError::StorageUnavailable)?;
    if system_identifier != target.system_identifier() {
        return Err(PlatformHostPreflightError::TargetMismatch);
    }
    let names = [
        config.namespaces().source(),
        config.namespaces().task(),
        config.namespaces().artifacts(),
    ];
    let rows = sqlx::query(
        "SELECT n.nspname::text AS name, n.oid::bigint AS oid,
        n.nspowner::bigint AS owner_oid,
        pg_catalog.has_schema_privilege(n.oid, 'USAGE') AS can_use,
        pg_catalog.has_schema_privilege(n.oid, 'CREATE') AS can_create
        FROM pg_catalog.pg_namespace n WHERE n.nspname IN ($1, $2, $3)
        ORDER BY n.nspname LIMIT 4",
    )
    .bind(names[0])
    .bind(names[1])
    .bind(names[2])
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() != 3 {
        return Err(PlatformHostPreflightError::UnsafeNamespace);
    }
    let mut namespaces = [None; 3];
    for row in rows {
        let name: String = row.try_get("name")?;
        let index = names
            .iter()
            .position(|expected| *expected == name)
            .ok_or(PlatformHostPreflightError::UnsafeNamespace)?;
        let identity = NamespaceIdentity {
            oid: oid(row.try_get("oid")?)?,
            owner_oid: oid(row.try_get("owner_oid")?)?,
        };
        if namespaces[index].is_some()
            || identity.owner_oid == role_oid
            || !row.try_get::<bool, _>("can_use")?
            || row.try_get::<bool, _>("can_create")?
        {
            return Err(PlatformHostPreflightError::UnsafeNamespace);
        }
        namespaces[index] = Some(identity);
    }
    let [Some(source), Some(task), Some(artifacts)] = namespaces else {
        return Err(PlatformHostPreflightError::UnsafeNamespace);
    };
    Ok(PlatformCatalogObservation {
        config: config.clone(),
        role_oid,
        database_owner_oid,
        namespaces: [source, task, artifacts],
    })
}

fn oid(value: i64) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or(PlatformHostPreflightError::StorageUnavailable)
}
