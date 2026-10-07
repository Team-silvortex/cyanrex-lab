//! Shared mechanics for the three trusted resource stores, not Session authorization.
use crate::sqlx_compat::{self as sqlx, postgres::PgRow, PgPool, Postgres, Row};
use sqlx_core::transaction::Transaction;

type Result<T> = std::result::Result<T, sqlx::Error>;

/// Begin only: callers still perform their namespace, schema and owner checks in order.
pub(super) async fn begin(pool: &PgPool, readonly: bool) -> Result<Transaction<'_, Postgres>> {
    let mut tx = pool.begin().await?;
    if readonly {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub(super) async fn namespace(tx: &mut Transaction<'_, Postgres>) -> Result<bool> {
    let row = sqlx::query(
        "SELECT current_schema() AS name, cardinality(current_schemas(false)) AS count",
    )
    .fetch_one(&mut **tx)
    .await?;
    let name: Option<String> = row.try_get("name")?;
    Ok(dedicated_namespace(name.as_deref(), row.try_get("count")?))
}

fn dedicated_namespace(name: Option<&str>, count: i32) -> bool {
    count == 1
        && name.is_some_and(|name| {
            name != "public" && name != "information_schema" && !name.starts_with("pg_")
        })
}

/// Callers retain their exact LOCK/SELECT statements and metadata/version/scope checks.
/// Preserve short-circuit decoding: an earlier invalid property is UnsupportedSchema at
/// the caller, while a decode failure actually reached remains its existing SQL error mapping.
pub(super) fn ordinary_tables(rows: &[PgRow], expected: usize) -> Result<bool> {
    if rows.len() != expected {
        return Ok(false);
    }
    for row in rows {
        if row.try_get::<String, _>("kind")? != "r"
            || row.try_get::<String, _>("persistence")? != "p"
            || row.try_get::<bool, _>("rls")?
            || row.try_get::<bool, _>("partition")?
            || row.try_get::<bool, _>("inheritance")?
            || !row.try_get::<bool, _>("resolved")?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlx_compat::PgPoolOptions;

    #[test]
    fn only_one_non_system_namespace_is_admitted() {
        assert!(dedicated_namespace(Some("private_tasks"), 1));
        assert!(dedicated_namespace(Some("Public"), 1));
        for name in [
            None,
            Some("public"),
            Some("information_schema"),
            Some("pg_temp_1"),
            Some("pg_catalog"),
        ] {
            assert!(!dedicated_namespace(name, 1));
        }
        for count in [-1, 0, 2, i32::MAX] {
            assert!(!dedicated_namespace(Some("private_tasks"), count));
        }
    }

    #[test]
    fn all_resource_stores_share_mechanics_without_moving_their_table_locks() {
        for (module, schema) in [
            (
                include_str!("task_store/mod.rs"),
                include_str!("task_store/schema.rs"),
            ),
            (
                include_str!("artifact_store/mod.rs"),
                include_str!("artifact_store/schema.rs"),
            ),
            (
                include_str!("review_store/mod.rs"),
                include_str!("review_store/schema.rs"),
            ),
        ] {
            assert!(module.contains("prepared_resource_sql::begin"));
            assert!(!module.contains("SET LOCAL statement_timeout"));
            assert!(schema.contains("prepared_resource_sql::namespace"));
            assert!(schema.contains("prepared_resource_sql::ordinary_tables"));
            let lock = schema.find("LOCK TABLE").unwrap();
            let shape = schema
                .find("prepared_resource_sql::ordinary_tables")
                .unwrap();
            let scope = schema.find("WHERE singleton FOR SHARE").unwrap();
            assert!(lock < shape && shape < scope);
        }
    }

    async fn pool() -> PgPool {
        let url = std::env::var("CYANREX_TEST_DATABASE_URL")
            .expect("set an explicit disposable CYANREX_TEST_DATABASE_URL");
        PgPoolOptions::new()
            .max_connections(1)
            .after_connect(|connection, _| {
                Box::pin(async move {
                    for statement in [
                        "SET default_transaction_isolation = 'read committed'",
                        "SET default_transaction_read_only = 'off'",
                        "SET statement_timeout = '0'",
                        "SET lock_timeout = '0'",
                    ] {
                        sqlx::query(statement).execute(&mut *connection).await?;
                    }
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap()
    }

    type Shape = (
        Option<&'static str>,
        Option<&'static str>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
        Option<bool>,
    );

    async fn shape_row(pool: &PgPool, shape: Shape) -> PgRow {
        sqlx::query(
            "SELECT $1::text AS kind, $2::text AS persistence, $3::bool AS rls,
            $4::bool AS partition, $5::bool AS inheritance, $6::bool AS resolved",
        )
        .bind(shape.0)
        .bind(shape.1)
        .bind(shape.2)
        .bind(shape.3)
        .bind(shape.4)
        .bind(shape.5)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_ordinary_table_predicates_and_decode_order_are_preserved() {
        let pool = pool().await;
        let healthy = (
            Some("r"),
            Some("p"),
            Some(false),
            Some(false),
            Some(false),
            Some(true),
        );
        let row = shape_row(&pool, healthy).await;
        assert!(ordinary_tables(&[row], 1).unwrap());
        let mut rejections = Vec::new();
        let mut value = healthy;
        value.0 = Some("v");
        rejections.push(value);
        let mut value = healthy;
        value.1 = Some("t");
        rejections.push(value);
        let mut value = healthy;
        value.2 = Some(true);
        rejections.push(value);
        let mut value = healthy;
        value.3 = Some(true);
        rejections.push(value);
        let mut value = healthy;
        value.4 = Some(true);
        rejections.push(value);
        let mut value = healthy;
        value.5 = Some(false);
        rejections.push(value);
        // Every rejected property short-circuits later nullable projections, in the old order.
        rejections.extend([
            (Some("v"), None, None, None, None, None),
            (Some("r"), Some("t"), None, None, None, None),
            (Some("r"), Some("p"), Some(true), None, None, None),
            (Some("r"), Some("p"), Some(false), Some(true), None, None),
            (
                Some("r"),
                Some("p"),
                Some(false),
                Some(false),
                Some(true),
                None,
            ),
        ]);
        for shape in rejections {
            let row = shape_row(&pool, shape).await;
            assert!(!ordinary_tables(&[row], 1).unwrap(), "{shape:?}");
        }
        let nullable = shape_row(&pool, (None, None, None, None, None, None)).await;
        assert!(!ordinary_tables(std::slice::from_ref(&nullable), 2).unwrap());
        assert!(!ordinary_tables(&[], 1).unwrap());
        assert!(matches!(
            ordinary_tables(&[nullable], 1),
            Err(sqlx::Error::ColumnDecode { .. })
        ));
        let mut null_resolved = healthy;
        null_resolved.5 = None;
        let row = shape_row(&pool, null_resolved).await;
        // Reached decode failures retain each existing store's SQL error classification.
        use crate::services::{review_store::ReviewStoreError, task_store::TaskStoreError};
        assert_eq!(
            TaskStoreError::from(ordinary_tables(std::slice::from_ref(&row), 1).unwrap_err()),
            TaskStoreError::StorageUnavailable
        );
        #[cfg(unix)]
        {
            use crate::services::artifact_store::ArtifactStoreError;
            assert_eq!(
                ArtifactStoreError::from(
                    ordinary_tables(std::slice::from_ref(&row), 1).unwrap_err()
                ),
                ArtifactStoreError::StorageUnavailable
            );
        }
        assert_eq!(
            ReviewStoreError::from(ordinary_tables(&[row], 1).unwrap_err()),
            ReviewStoreError::StorageUnavailable
        );
        pool.close().await;
    }

    async fn settings(
        connection: &mut sqlx_postgres::PgConnection,
    ) -> (String, String, String, String) {
        let row = sqlx::query(
            "SELECT current_setting('transaction_isolation') AS isolation,
            current_setting('transaction_read_only') AS readonly,
            current_setting('statement_timeout') AS statement,
            current_setting('lock_timeout') AS lock",
        )
        .fetch_one(connection)
        .await
        .unwrap();
        (
            row.get("isolation"),
            row.get("readonly"),
            row.get("statement"),
            row.get("lock"),
        )
    }

    #[tokio::test]
    #[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_transaction_modes_and_local_limits_do_not_leak_after_rollback() {
        let pool = pool().await;
        let mut read = begin(&pool, true).await.unwrap();
        let read_settings = settings(&mut read).await;
        read.rollback().await.unwrap();
        let after_read = settings(&mut pool.acquire().await.unwrap()).await;
        let mut write = begin(&pool, false).await.unwrap();
        let write_settings = settings(&mut write).await;
        write.rollback().await.unwrap();
        let after_write = settings(&mut pool.acquire().await.unwrap()).await;
        let cancelled = begin(&pool, true).await.unwrap();
        drop(cancelled);
        let after_drop = settings(&mut pool.acquire().await.unwrap()).await;
        pool.close().await;
        let defaults: (String, String, String, String) = (
            "read committed".into(),
            "off".into(),
            "0".into(),
            "0".into(),
        );
        assert_eq!(
            read_settings,
            (
                "repeatable read".to_string(),
                "on".to_string(),
                "5s".to_string(),
                "2s".to_string()
            )
        );
        assert_eq!(
            write_settings,
            (
                "read committed".to_string(),
                "off".to_string(),
                "5s".to_string(),
                "2s".to_string()
            )
        );
        assert_eq!(after_read, defaults);
        assert_eq!(after_write, defaults);
        assert_eq!(after_drop, defaults);
    }
}
