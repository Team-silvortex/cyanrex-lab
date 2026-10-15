use super::{
    catalog, PlatformCatalogObservation, PlatformHostPreflightConfig, PlatformHostPreflightError,
    Result,
};
use crate::sqlx_compat::{self as sqlx, Postgres, Row};
use sqlx_core::{connection::Connection, transaction::Transaction};
use sqlx_postgres::PgConnection;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Scope {
    Version,
    Authority,
    Workspace,
}

struct Marker {
    namespace: usize,
    table: &'static str,
    version: &'static str,
    scope: Scope,
}

// Fixed lock/read order. The Source namespace includes both registry version markers.
const MARKERS: [Marker; 5] = [
    Marker {
        namespace: 0,
        table: "collaboration_auth_source_schema",
        version: "2",
        scope: Scope::Authority,
    },
    Marker {
        namespace: 0,
        table: "collaboration_identity_schema",
        version: "2",
        scope: Scope::Version,
    },
    Marker {
        namespace: 0,
        table: "collaboration_access_schema",
        version: "2",
        scope: Scope::Version,
    },
    Marker {
        namespace: 1,
        table: "collaboration_task_schema",
        version: "3",
        scope: Scope::Workspace,
    },
    Marker {
        namespace: 2,
        table: "collaboration_artifact_schema",
        version: "1",
        scope: Scope::Workspace,
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
struct MarkerIdentity {
    oid: u32,
    owner_oid: u32,
}

/// Opaque marker observation, not complete schema compatibility, business health or authority.
#[derive(PartialEq, Eq)]
pub struct PlatformStorageMarkerObservation {
    catalog: PlatformCatalogObservation,
    markers: [MarkerIdentity; 5],
}

impl PlatformStorageMarkerObservation {
    pub(super) fn catalog(&self) -> &PlatformCatalogObservation {
        &self.catalog
    }

    /// Passive metadata comparison only; does not open storage or revalidate a live target.
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

/// Observe only five version/scope markers under an original catalog observation.
/// Candidate marker names are locked before the first catalog SELECT; they are not read before
/// physical-target verification. This rejects outer transactions with the catalog's original
/// boundary probe and owns one RR READ ONLY transaction, never a caller's snapshot.
/// Once acquired, an owned SQLx transaction queues rollback on cancellation; interrupted BEGIN
/// or probe completion remains uncertain. No deadline proves that an unreachable server stopped.
pub async fn inspect_platform_storage_markers(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected_catalog: &PlatformCatalogObservation,
) -> Result<PlatformStorageMarkerObservation> {
    if !expected_catalog.config_matches(config) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    if connection.is_in_transaction() {
        return Err(PlatformHostPreflightError::InvalidConfig);
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        catalog::require_idle(connection).await?;
        observe_owned(config, connection, expected_catalog).await
    })
    .await
    .map_err(|_| PlatformHostPreflightError::StorageUnavailable)?
}

/// Recheck original marker identities without adopting replacements or installing storage.
pub async fn verify_platform_storage_markers(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected: &PlatformStorageMarkerObservation,
) -> Result<()> {
    if !expected.catalog.config_matches(config) {
        return Err(PlatformHostPreflightError::StorageMarkersChanged);
    }
    let current = inspect_platform_storage_markers(config, connection, &expected.catalog).await?;
    if !expected.matches(&current) {
        return Err(PlatformHostPreflightError::StorageMarkersChanged);
    }
    Ok(())
}

async fn observe_owned(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected_catalog: &PlatformCatalogObservation,
) -> Result<PlatformStorageMarkerObservation> {
    let mut tx = connection
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?;
    let observed = collect(config, &mut tx, expected_catalog).await;
    match observed {
        Ok(observation) => {
            tx.commit().await?;
            Ok(observation)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

fn namespaces(config: &PlatformHostPreflightConfig) -> [&str; 3] {
    [
        config.namespaces().source(),
        config.namespaces().task(),
        config.namespaces().artifacts(),
    ]
}

fn qualified(config: &PlatformHostPreflightConfig, marker: &Marker) -> String {
    // Only validated restricted identifiers and fixed table names become SQL identifiers.
    format!(
        "\"{}\".\"{}\"",
        namespaces(config)[marker.namespace],
        marker.table
    )
}

async fn collect(
    config: &PlatformHostPreflightConfig,
    tx: &mut Transaction<'_, Postgres>,
    expected_catalog: &PlatformCatalogObservation,
) -> Result<PlatformStorageMarkerObservation> {
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut **tx)
        .await?;
    // Fail rather than evaluate an RLS policy or treat filtered rows as absent. This does not
    // bypass RLS, elevate this role, or change the session's setting outside our transaction.
    sqlx::query("SET LOCAL row_security = off")
        .execute(&mut **tx)
        .await?;
    let candidates = MARKERS
        .iter()
        .map(|marker| format!("ONLY {}", qualified(config, marker)))
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(&format!("LOCK TABLE {candidates} IN ACCESS SHARE MODE"))
        .execute(&mut **tx)
        .await
        .map_err(marker_error)?;
    // All five relation locks precede this first catalog SELECT. The locks then protect their
    // structure through readback and COMMIT; pg_locks/name/OID checks below reject mis-resolution.
    let observed_catalog = catalog::collect(config, tx).await?;
    if !expected_catalog.matches(&observed_catalog) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    let namespace_oids = observed_catalog.namespace_oids();
    let mut markers = Vec::with_capacity(MARKERS.len());
    for marker in &MARKERS {
        let identity = shape(config, tx, marker, namespace_oids[marker.namespace]).await?;
        read_marker(config, tx, marker, identity).await?;
        if shape(config, tx, marker, namespace_oids[marker.namespace]).await? != identity {
            return Err(PlatformHostPreflightError::StorageMarkersChanged);
        }
        markers.push(identity);
    }
    if !expected_catalog.matches(&catalog::collect(config, tx).await?) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    let markers = markers
        .try_into()
        .map_err(|_| PlatformHostPreflightError::UnsupportedStorageMarker)?;
    Ok(PlatformStorageMarkerObservation {
        catalog: observed_catalog,
        markers,
    })
}

async fn shape(
    config: &PlatformHostPreflightConfig,
    tx: &mut Transaction<'_, Postgres>,
    marker: &Marker,
    namespace_oid: u32,
) -> Result<MarkerIdentity> {
    let mut fields = "('singleton', 'pg_catalog.bool'::regtype::oid), ('version', 'pg_catalog.int4'::regtype::oid)".to_owned();
    if !matches!(marker.scope, Scope::Version) {
        fields.push_str(", ('authority_id', 'pg_catalog.uuid'::regtype::oid)");
    }
    if matches!(marker.scope, Scope::Workspace) {
        fields.push_str(", ('workspace_id', 'pg_catalog.uuid'::regtype::oid)");
    }
    let statement = format!("WITH expected(name, type_oid) AS (VALUES {fields})
        SELECT c.oid::bigint AS oid, c.relowner::bigint AS owner_oid,
        pg_catalog.to_regnamespace($1)::oid::bigint AS resolved_namespace,
        pg_catalog.to_regclass($2)::oid::bigint AS resolved_table,
        c.relkind = 'r' AND c.relpersistence = 'p' AND NOT c.relrowsecurity
            AND NOT c.relforcerowsecurity AND NOT c.relispartition AND NOT c.relhassubclass
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_inherits i
                WHERE i.inhrelid = c.oid OR i.inhparent = c.oid) AS ordinary,
        c.relowner <> (SELECT r.oid FROM pg_catalog.pg_roles r WHERE r.rolname = current_user)
            AND NOT pg_catalog.has_table_privilege(c.oid, 'TRIGGER') AS limited_role,
        EXISTS (SELECT 1 FROM pg_catalog.pg_locks l WHERE l.locktype = 'relation'
            AND l.pid = pg_catalog.pg_backend_pid() AND l.database = $4::oid
            AND l.relation = c.oid AND l.mode = 'AccessShareLock' AND l.granted) AS held,
        NOT EXISTS (SELECT 1 FROM expected e LEFT JOIN pg_catalog.pg_attribute a
            ON a.attrelid = c.oid AND a.attname = e.name AND NOT a.attisdropped
            WHERE a.attnum IS NULL OR a.attnum <= 0 OR a.atttypid <> e.type_oid
                OR NOT a.attnotnull OR a.attgenerated <> '' OR a.attidentity <> '')
            AND (SELECT count(*) FROM pg_catalog.pg_attribute a WHERE a.attrelid = c.oid
                AND a.attnum > 0 AND NOT a.attisdropped) = (SELECT count(*) FROM expected) AS columns_valid,
        EXISTS (SELECT 1 FROM pg_catalog.pg_constraint k WHERE k.conrelid = c.oid
            AND k.contype = 'p' AND k.convalidated AND NOT k.condeferrable
            AND k.conislocal AND k.coninhcount = 0
            AND k.conkey = ARRAY(SELECT a.attnum FROM pg_catalog.pg_attribute a
                WHERE a.attrelid = c.oid AND a.attname = 'singleton'
                    AND NOT a.attisdropped AND a.attnotnull)::smallint[]) AS key_valid
        FROM pg_catalog.pg_class c WHERE c.relnamespace = $3::oid AND c.relname = $5 LIMIT 2");
    let rows = sqlx::query(&statement)
        .bind(namespaces(config)[marker.namespace])
        .bind(qualified(config, marker))
        .bind(i64::from(namespace_oid))
        .bind(i64::from(config.target().database_oid()))
        .bind(marker.table)
        .fetch_all(&mut **tx)
        .await
        .map_err(marker_error)?;
    if rows.len() != 1 {
        return Err(PlatformHostPreflightError::UnsupportedStorageMarker);
    }
    let row = &rows[0];
    let identity = MarkerIdentity {
        oid: oid(row.try_get("oid")?)?,
        owner_oid: oid(row.try_get("owner_oid")?)?,
    };
    if row.try_get::<Option<i64>, _>("resolved_namespace")? != Some(i64::from(namespace_oid))
        || row.try_get::<Option<i64>, _>("resolved_table")? != Some(i64::from(identity.oid))
        || !row.try_get::<bool, _>("held")?
    {
        return Err(PlatformHostPreflightError::StorageMarkersChanged);
    }
    for flag in ["ordinary", "limited_role", "columns_valid", "key_valid"] {
        if !row.try_get::<Option<bool>, _>(flag)?.unwrap_or(false) {
            return Err(PlatformHostPreflightError::UnsupportedStorageMarker);
        }
    }
    Ok(identity)
}

async fn read_marker(
    config: &PlatformHostPreflightConfig,
    tx: &mut Transaction<'_, Postgres>,
    marker: &Marker,
    identity: MarkerIdentity,
) -> Result<()> {
    let mut types = "pg_catalog.pg_typeof(t.singleton) = 'pg_catalog.bool'::regtype
        AND pg_catalog.pg_typeof(t.version) = 'pg_catalog.int4'::regtype"
        .to_owned();
    let mut scope = "TRUE".to_owned();
    if !matches!(marker.scope, Scope::Version) {
        types.push_str(" AND pg_catalog.pg_typeof(t.authority_id) = 'pg_catalog.uuid'::regtype");
        scope =
            "COALESCE(CASE WHEN pg_catalog.pg_typeof(t.authority_id) = 'pg_catalog.uuid'::regtype
            THEN t.authority_id::text = e.authority_id ELSE false END, false)"
                .to_owned();
    }
    if matches!(marker.scope, Scope::Workspace) {
        types.push_str(" AND pg_catalog.pg_typeof(t.workspace_id) = 'pg_catalog.uuid'::regtype");
        scope.push_str(" AND COALESCE(CASE WHEN pg_catalog.pg_typeof(t.workspace_id) = 'pg_catalog.uuid'::regtype
            THEN t.workspace_id::text = e.workspace_id ELSE false END, false)");
    }
    let statement = format!(
        "WITH expected AS
        (SELECT $1::text AS version, $2::text AS authority_id, $3::text AS workspace_id)
        SELECT t.tableoid::oid::bigint AS table_oid, ({types}) AS types_valid,
        COALESCE(CASE WHEN pg_catalog.pg_typeof(t.singleton) = 'pg_catalog.bool'::regtype
            THEN t.singleton::text = 'true' ELSE false END, false) AS singleton_valid,
        COALESCE(CASE WHEN pg_catalog.pg_typeof(t.version) = 'pg_catalog.int4'::regtype
            THEN t.version::text = e.version ELSE false END, false) AS version_valid,
        ({scope}) AS scope_valid FROM ONLY {} t CROSS JOIN expected e LIMIT 2",
        qualified(config, marker)
    );
    let rows = sqlx::query(&statement)
        .bind(marker.version)
        .bind(config.scope().authority_id.to_string())
        .bind(config.scope().workspace_id.to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(marker_error)?;
    if rows.len() != 1 {
        return Err(PlatformHostPreflightError::UnsupportedStorageMarker);
    }
    let row = &rows[0];
    if oid(row.try_get("table_oid")?)? != identity.oid {
        return Err(PlatformHostPreflightError::StorageMarkersChanged);
    }
    for flag in ["types_valid", "singleton_valid", "version_valid"] {
        if !row.try_get::<Option<bool>, _>(flag)?.unwrap_or(false) {
            return Err(PlatformHostPreflightError::UnsupportedStorageMarker);
        }
    }
    if !row
        .try_get::<Option<bool>, _>("scope_valid")?
        .unwrap_or(false)
    {
        return Err(PlatformHostPreflightError::StorageScopeMismatch);
    }
    Ok(())
}

fn oid(value: i64) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or(PlatformHostPreflightError::UnsupportedStorageMarker)
}

fn marker_error(error: sqlx::Error) -> PlatformHostPreflightError {
    match error
        .as_database_error()
        .and_then(|error| error.code())
        .as_deref()
    {
        Some("42P01" | "3F000" | "42703" | "42809") => {
            PlatformHostPreflightError::UnsupportedStorageMarker
        }
        _ => PlatformHostPreflightError::StorageUnavailable,
    }
}
