use super::{
    catalog_fixture::{within, Fixture},
    marker_fixture::{prepare, snapshot},
    support::{self, attempt, binding, no_connections, Mode, Root, PASSWORD},
};
use cyanrex_engine::platform_host::{
    PlatformDatabaseConnector, PlatformDatabaseEndpoint, PlatformHostPreflightConfig,
    PlatformStorageConnectionError, PlatformStorageConnector,
};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::{
    ffi::CString,
    fs::{self, File},
    io::{self, Read},
    net::{IpAddr, Ipv4Addr},
    os::{
        fd::FromRawFd,
        unix::{ffi::OsStrExt, fs::PermissionsExt},
    },
    path::Path,
    time::Duration,
};
use tokio::net::TcpListener;

// Only this owned unchanged root is watched, and only IN_OPEN is requested. The two reads
// below separate the known precheck from the postcheck after the accepted peer closes.
struct RootOpens(File);
impl RootOpens {
    fn new(path: &Path) -> Self {
        // SAFETY: successful init creates one owned descriptor, transferred exactly once.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        assert!(fd >= 0);
        let file = unsafe { File::from_raw_fd(fd) };
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: fd remains live and path is an owned NUL-terminated string.
        assert!(unsafe { libc::inotify_add_watch(fd, path.as_ptr(), libc::IN_OPEN) } >= 0);
        Self(file)
    }

    fn observed(&mut self) -> bool {
        let mut bytes = [0; 8192];
        match self.0.read(&mut bytes) {
            Ok(length) => length != 0,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
            Err(_) => panic!("owned root-open oracle unavailable"),
        }
    }
}

async fn probe_binding(
    fixture: &Fixture,
    root: &Root,
    path: &Path,
    config: &PlatformHostPreflightConfig,
    port: u16,
) -> Result<PlatformStorageConnector, PlatformStorageConnectionError> {
    let (catalog, markers) = support::catalog_and_markers(fixture).await;
    let endpoint =
        PlatformDatabaseEndpoint::loopback(IpAddr::V4(Ipv4Addr::LOCALHOST), port).unwrap();
    let database =
        PlatformDatabaseConnector::new(fixture.config().clone(), catalog, endpoint, PASSWORD)
            .unwrap_or_else(|_| panic!("owned probe database connector unavailable"));
    PlatformStorageConnector::new(
        config.clone(),
        database,
        markers,
        path,
        root.original(fixture.config()),
    )
}

async fn unavailable(connector: &PlatformStorageConnector) -> bool {
    match tokio::time::timeout(Duration::from_millis(800), connector.connect()).await {
        Ok(Err(PlatformStorageConnectionError::Unavailable)) => true,
        Ok(Ok(connection)) => {
            let _ = within(connection.close()).await;
            false
        }
        _ => false,
    }
}

async fn no_dial(listener: &TcpListener) -> bool {
    match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
        Err(_) => true,
        Ok(Ok((socket, _))) => {
            drop(socket);
            false
        }
        _ => false,
    }
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_connection_original_root_precheck_rejects_before_dial_and_never_adopts_replacement(
) {
    let (
        constructors,
        replaced,
        swap_rejected,
        unsafe_rejected,
        no_attempts,
        positive,
        restored,
        healthy,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let root = Root::new();
            let before = snapshot(fixture).await;
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let altered = PlatformHostPreflightConfig::new(
                "https://different-platform.example",
                fixture.config().scope(),
                fixture.config().namespaces().clone(),
                fixture.config().target().clone(),
                fixture.config().limits(),
            )
            .unwrap();
            let raw_alias = root.path().join(".");
            let different = root.path().with_file_name("unobserved-artifacts");
            let mut constructors = Vec::new();
            for (path, config) in [
                (raw_alias.as_path(), fixture.config()),
                (different.as_path(), fixture.config()),
                (root.path(), &altered),
            ] {
                constructors.push(matches!(
                    probe_binding(fixture, &root, path, config, port).await,
                    Err(PlatformStorageConnectionError::InvalidConfig)
                ));
            }
            let connector = probe_binding(fixture, &root, root.path(), fixture.config(), port)
                .await
                .unwrap_or_else(|_| panic!("owned original root connector unavailable"));
            let replaced = root.swap();
            let swap_rejected = unavailable(&connector).await;
            let mut no_attempts = no_dial(&listener).await;
            let restored_swap = root.restore();
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o750)).unwrap();
            let unsafe_rejected = unavailable(&connector).await;
            no_attempts &= no_dial(&listener).await;
            let unsafe_unrepaired =
                fs::metadata(root.path()).unwrap().permissions().mode() & 0o7777 == 0o750;
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();

            // Exact restoration must allow a real attempt. The probe is a real TCP peer,
            // not PostgreSQL authentication or a successful storage check.
            let mut opens = RootOpens::new(root.path());
            let (result, (accepted, precheck_opened)) = tokio::join!(connector.connect(), async {
                match tokio::time::timeout(Duration::from_secs(2), listener.accept()).await {
                    Ok(Ok((socket, _))) => {
                        let precheck_opened = opens.observed();
                        drop(socket);
                        (true, precheck_opened)
                    }
                    _ => (false, false),
                }
            });
            let post_failure_opened = opens.observed();
            drop(opens);
            let failed_probe = match result {
                Err(PlatformStorageConnectionError::Unavailable) => true,
                Ok(connection) => {
                    let _ = within(connection.close()).await;
                    false
                }
                _ => false,
            };
            let positive = accepted && failed_probe && precheck_opened && post_failure_opened;
            no_attempts &= no_dial(&listener).await;
            drop(listener);
            drop(connector);
            let healthy_connector = binding(fixture, &root, Mode::Observed)
                .await
                .unwrap_or_else(|_| panic!("owned restored storage connector unavailable"));
            let healthy = attempt(&healthy_connector).await.is_ok();
            drop(healthy_connector);
            let no_backend = no_connections(fixture).await;
            let unchanged = before == snapshot(fixture).await;
            let cleaned = root.finish();
            (
                constructors,
                replaced,
                swap_rejected,
                unsafe_rejected && unsafe_unrepaired,
                no_attempts,
                positive,
                restored_swap,
                healthy,
                unchanged,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(constructors.len() == 3 && constructors.into_iter().all(|rejected| rejected));
    assert!(replaced && swap_rejected && unsafe_rejected && no_attempts && positive);
    assert!(restored && healthy && unchanged && no_backend && cleaned);
}

async fn lock_marker(fixture: &Fixture) -> (PgConnection, i64, i32) {
    let oid = within(
        query(
            "SELECT c.oid::bigint AS oid FROM pg_catalog.pg_class c
        JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relname='collaboration_auth_source_schema'",
        )
        .bind(&fixture.source)
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap()
    .get("oid");
    let mut connection = within(PgConnection::connect_with(
        fixture.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap_or_else(|_| panic!("owned independent marker holder unavailable"));
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query(&format!(
            "LOCK TABLE {}.collaboration_auth_source_schema IN ACCESS EXCLUSIVE MODE",
            fixture.source
        ))
        .execute(&mut connection),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, oid, pid)
}

async fn pending_marker_lock(fixture: &Fixture, oid: i64, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_millis(900), async {
        loop {
            let row = query(
                "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_locks l
                JOIN pg_catalog.pg_stat_activity a ON a.pid=l.pid
                WHERE a.datname=$1 AND a.usename=$2
                AND a.application_name='cyanrex-platform-connection'
                AND l.database=$3::oid AND l.relation=$4::oid
                AND l.locktype='relation' AND l.mode='AccessShareLock' AND NOT l.granted
                AND $5=ANY(pg_catalog.pg_blocking_pids(a.pid))) AS pending",
            )
            .bind(&fixture.database)
            .bind(&fixture.role)
            .bind(i64::from(fixture.config().target().database_oid()))
            .bind(oid)
            .bind(blocker)
            .fetch_one(fixture.owner())
            .await;
            if row.is_ok_and(|row| row.get::<bool, _>("pending")) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

async fn release(mut connection: PgConnection) -> bool {
    let rolled_back = within(query("ROLLBACK").execute(&mut connection))
        .await
        .is_ok();
    let closed = within(connection.close()).await.is_ok();
    rolled_back && closed
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_connection_original_root_postcheck_rejects_swap_during_real_marker_wait(
) {
    let (
        pending,
        replaced,
        released,
        rejected,
        restored,
        recovered,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let root = Root::new();
            let connector = binding(fixture, &root, Mode::Expected)
                .await
                .unwrap_or_else(|_| panic!("owned expected storage connector unavailable"));
            let before = snapshot(fixture).await;
            let (holder, oid, pid) = lock_marker(fixture).await;
            let (result, (pending, replaced, released)) =
                tokio::join!(connector.connect(), async {
                    let pending = pending_marker_lock(fixture, oid, pid).await;
                    let replaced = pending && root.swap();
                    // Always release/close the independent holder before checking outcomes.
                    let released = release(holder).await;
                    (pending, replaced, released)
                });
            let rejected = match result {
                Err(PlatformStorageConnectionError::Unavailable) => true,
                Ok(connection) => {
                    let _ = within(connection.close()).await;
                    false
                }
                _ => false,
            };
            let no_failed_backend = no_connections(fixture).await;
            let restored = root.restore();
            let recovered = attempt(&connector).await.is_ok();
            // Cancel only after an independent observer identifies this authenticated
            // connection's real marker wait. Dropping the owned future must not publish it.
            let (holder, oid, pid) = lock_marker(fixture).await;
            let mut waiting = Box::pin(connector.connect());
            let cancel_pending = tokio::select! {
                result = &mut waiting => {
                    if let Ok(connection) = result { let _ = within(connection.close()).await; }
                    false
                }
                pending = pending_marker_lock(fixture, oid, pid) => pending,
            };
            drop(waiting);
            let cancel_released = release(holder).await;
            let cancel_no_backend = no_connections(fixture).await;
            let recovered = attempt(&connector).await.is_ok() && recovered;
            drop(connector);
            let no_backend =
                no_failed_backend && cancel_no_backend && no_connections(fixture).await;
            let unchanged = before == snapshot(fixture).await;
            let cleaned = root.finish();
            (
                pending && cancel_pending,
                replaced,
                released && cancel_released,
                rejected,
                restored,
                recovered,
                unchanged,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    assert!(
        pending,
        "this connector must wait on the independently identified marker holder"
    );
    assert!(replaced && released && rejected && restored && recovered);
    assert!(unchanged && no_backend && cleaned);
}
