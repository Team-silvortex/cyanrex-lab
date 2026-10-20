pub(super) use super::{
    catalog_fixture::{within, Fixture},
    expectation_helpers::{config_variants, quote_identifier, Pins},
    marker_fixture::{prepare, snapshot},
    marker_helpers::{current, relation, MarkerPins, PASSWORD, TABLES},
    storage_helpers::{endpoint, no_connections, Root},
};
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformStorageConnectionError, PlatformStorageConnector,
};
use sqlx_core::{connection::Connection, query::query, row::Row};
use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinHandle};

pub(super) async fn open(
    f: &Fixture,
    root: &Root,
    catalog: Pins,
    markers: MarkerPins,
) -> Result<PlatformStorageConnector, PlatformStorageConnectionError> {
    within(open_platform_storage_expected(
        f.config().clone(),
        endpoint(f),
        PASSWORD,
        markers.expectation(f, catalog),
        root.path(),
        root.original(f.config()),
    ))
    .await
}

pub(super) async fn usable(f: &Fixture, binding: &PlatformStorageConnector) -> bool {
    match within(binding.connect()).await {
        Ok(mut connection) => {
            let (_, exact) = super::storage_helpers::pid(f, &mut connection).await;
            let closed = within(connection.close()).await.is_ok();
            exact && closed
        }
        Err(_) => false,
    }
}

pub(super) async fn probe() -> (
    TcpListener,
    cyanrex_engine::platform_host::PlatformDatabaseEndpoint,
) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let endpoint = cyanrex_engine::platform_host::PlatformDatabaseEndpoint::loopback(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        listener.local_addr().unwrap().port(),
    )
    .unwrap();
    (listener, endpoint)
}

pub(super) async fn never_dialed(listener: TcpListener) -> bool {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => true,
        Ok(Ok((connection, _))) => {
            drop(connection);
            false
        }
        Ok(Err(_)) => false,
    }
}

pub(super) struct Pending<T>(Option<JoinHandle<T>>);
impl<T: Send + 'static> Pending<T> {
    pub(super) fn start(future: impl Future<Output = T> + Send + 'static) -> Self {
        Self(Some(tokio::spawn(future)))
    }
    pub(super) async fn cancel(mut self) -> bool {
        self.0.as_ref().unwrap().abort();
        let result = within(self.0.as_mut().unwrap()).await;
        self.0.take();
        result.is_err_and(|error| error.is_cancelled())
    }
}
impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if let Some(job) = &self.0 {
            job.abort();
        }
    }
}

pub(super) async fn replace_marker(f: &Fixture, index: usize) {
    let name = relation(f, index);
    let schema = match index {
        0..=2 => &f.source,
        3 => &f.task,
        _ => &f.artifacts,
    };
    let saved = format!("{}_open_saved", TABLES[index]);
    f.sql(&format!("ALTER TABLE {name} RENAME TO {saved}"))
        .await;
    f.sql(&format!(
        "CREATE TABLE {name} (LIKE {schema}.{saved} INCLUDING ALL)"
    ))
    .await;
    f.sql(&format!(
        "INSERT INTO {name} SELECT * FROM {schema}.{saved}"
    ))
    .await;
    f.sql(&format!("GRANT SELECT ON {name} TO {}", f.role))
        .await;
}

pub(super) async fn restore_marker(f: &Fixture, index: usize) {
    let schema = match index {
        0..=2 => &f.source,
        3 => &f.task,
        _ => &f.artifacts,
    };
    f.sql(&format!("DROP TABLE {}", relation(f, index))).await;
    f.sql(&format!(
        "ALTER TABLE {schema}.{}_open_saved RENAME TO {}",
        TABLES[index], TABLES[index]
    ))
    .await;
}

// Keep every table's original OID while replacing a namespace OID. The markers retain the
// same names, owners and rows, so the current diagnostic is a meaningful positive control.
pub(super) async fn replace_namespace(f: &Fixture, index: usize) {
    let schema = [&f.source, &f.task, &f.artifacts][index];
    let tables: Vec<String> = within(
        query(
            "SELECT c.relname::text AS name FROM pg_catalog.pg_class c
        JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relkind='r' ORDER BY c.relname",
        )
        .bind(schema)
        .fetch_all(f.owner()),
    )
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.get("name"))
    .collect();
    f.sql(&format!(
        "ALTER SCHEMA {schema} RENAME TO {schema}_open_saved"
    ))
    .await;
    f.sql(&format!("CREATE SCHEMA {schema}")).await;
    f.sql(&format!("REVOKE ALL ON SCHEMA {schema} FROM PUBLIC"))
        .await;
    f.sql(&format!("GRANT USAGE ON SCHEMA {schema} TO {}", f.role))
        .await;
    for table in tables {
        f.sql(&format!(
            "ALTER TABLE {schema}_open_saved.{table} SET SCHEMA {schema}"
        ))
        .await;
    }
}

pub(super) async fn restore_namespace(f: &Fixture, index: usize) {
    let schema = [&f.source, &f.task, &f.artifacts][index];
    let tables: Vec<String> = within(
        query(
            "SELECT c.relname::text AS name FROM pg_catalog.pg_class c
        JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relkind='r' ORDER BY c.relname",
        )
        .bind(schema)
        .fetch_all(f.owner()),
    )
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.get("name"))
    .collect();
    for table in tables {
        f.sql(&format!(
            "ALTER TABLE {schema}.{table} SET SCHEMA {schema}_open_saved"
        ))
        .await;
    }
    f.sql(&format!("DROP SCHEMA {schema}")).await;
    f.sql(&format!(
        "ALTER SCHEMA {schema}_open_saved RENAME TO {schema}"
    ))
    .await;
}
