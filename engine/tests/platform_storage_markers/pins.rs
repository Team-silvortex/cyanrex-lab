use super::{
    control::{wait_for_relation_lock, Admin, Pending},
    fixture::{prepare, snapshot, within, Fixture},
    support::*,
};
use cyanrex_engine::platform_host::{
    inspect_platform_storage_markers, verify_platform_storage_markers,
    PlatformHostPreflightError as Error,
};
use futures_util::FutureExt;
use sqlx_core::{connection::Connection, query::query, row::Row};
use std::panic::AssertUnwindSafe;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_markers_refuse_waited_replacement_type_and_row_security_drift() {
    let results = Fixture::checked(|fixture| Box::pin(async move {
        prepare(fixture).await;
        let all = markers(fixture);
        let mut scenarios: Vec<_> = all.iter().cloned().map(|marker| ("replacement", marker)).collect();
        scenarios.extend([("type", all[3].clone()), ("nullable", all[0].clone()),
            ("primary_key", all[4].clone()), ("row_security", all[2].clone())]);
        let mut results = Vec::new();
        for (fault, marker) in scenarios {
            let original_catalog = catalog(fixture).await;
            let mut connection = within(fixture.runtime().acquire()).await
                .unwrap_or_else(|_| panic!("original storage observation connection unavailable"));
            let original_storage = within(inspect_platform_storage_markers(fixture.config(), &mut connection, &original_catalog)).await
                .unwrap_or_else(|_| panic!("healthy original storage observation unavailable"));
            drop(connection);
            let before = snapshot(fixture).await;
            let oid = relation_oid(fixture, &marker).await;
            let key: String = within(query("SELECT conname::text AS name FROM pg_catalog.pg_constraint
                WHERE conrelid=pg_catalog.to_regclass($1) AND contype='p'")
                .bind(marker.name()).fetch_one(fixture.owner())).await
                .unwrap_or_else(|_| panic!("original marker primary key unavailable")).get("name");
            let mut admin = Admin::open(fixture).await;
            let blocker = admin.pid;
            // A failed oracle drops the fence and aborts its waiter; Pending::Drop does not
            // synchronously join it. Below we close/observe the independent backend, and the
            // owning Fixture awaits runtime-pool closure before dropping its database/role.
            let outcome = AssertUnwindSafe(async {
                let mut fence = within(admin.connection().begin()).await
                    .unwrap_or_else(|_| panic!("owned marker fence transaction unavailable"));
                within(query(&format!("LOCK TABLE {} IN ACCESS EXCLUSIVE MODE", marker.name()))
                    .execute(&mut *fence)).await
                    .unwrap_or_else(|_| panic!("owned marker ACCESS EXCLUSIVE fence failed"));
                let config = fixture.config().clone();
                let pool = fixture.runtime().clone();
                let (started, start) = tokio::sync::oneshot::channel();
                let pending = Pending::start(async move {
                    let mut connection = within(pool.acquire()).await
                        .unwrap_or_else(|_| panic!("owned waited marker connection unavailable"));
                    let pid: i32 = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut *connection)).await
                        .unwrap_or_else(|_| panic!("owned waited marker identity unavailable")).get("pid");
                    let _ = started.send(pid);
                    let result = if fault == "replacement" {
                        verify_platform_storage_markers(&config, &mut connection, &original_storage).await
                    } else {
                        inspect_platform_storage_markers(&config, &mut connection, &original_catalog).await.map(|_| ())
                    };
                    (result, original_catalog, original_storage)
                });
                let waiter = within(start).await
                    .unwrap_or_else(|_| panic!("owned marker waiter did not start"));
                let blocked = wait_for_relation_lock(fixture, waiter, blocker, oid).await;
                let retained = format!("retained_{}", marker.table);
                let statements = match fault {
                    "replacement" => vec![
                        format!("ALTER TABLE {} RENAME TO {retained}", marker.name()),
                        format!("CREATE TABLE {} (LIKE {}.{retained} INCLUDING ALL)", marker.name(), marker.schema),
                        format!("INSERT INTO {} SELECT * FROM {}.{retained}", marker.name(), marker.schema),
                        format!("GRANT SELECT ON {} TO {}", marker.name(), fixture.role),
                    ],
                    "type" => vec![format!("ALTER TABLE {} ALTER COLUMN version TYPE text USING version::text", marker.name())],
                    "nullable" => vec![format!("ALTER TABLE {} ALTER COLUMN version DROP NOT NULL", marker.name())],
                    "primary_key" => vec![format!("ALTER TABLE {} DROP CONSTRAINT {key}", marker.name())],
                    "row_security" => vec![format!("ALTER TABLE {} ENABLE ROW LEVEL SECURITY", marker.name())],
                    _ => unreachable!(),
                };
                for statement in statements {
                    within(query(&statement).execute(&mut *fence)).await
                        .unwrap_or_else(|_| panic!("owned waited marker drift setup failed"));
                }
                within(fence.commit()).await
                    .unwrap_or_else(|_| panic!("owned waited marker drift commit failed"));
                let current_oid = relation_oid(fixture, &marker).await;
                let fault_identity_proven = if fault == "replacement" { current_oid != oid } else { current_oid == oid };
                let (result, original_catalog, original_storage) = pending.finish().await;
                let mut replacement_ok = true;
                if fault == "replacement" {
                    let fresh_oid = relation_oid(fixture, &marker).await;
                    let mut connection = within(fixture.runtime().acquire()).await
                        .unwrap_or_else(|_| panic!("fresh replacement observation connection unavailable"));
                    let fresh = within(inspect_platform_storage_markers(fixture.config(), &mut connection, &original_catalog)).await;
                    replacement_ok = fresh_oid != oid && fresh.as_ref().is_ok_and(|fresh|
                        !fresh.matches(&original_storage) && !original_storage.matches(fresh));
                    drop(fresh);
                    drop(connection);
                    fixture.sql(&format!("DROP TABLE {}", marker.name())).await;
                    fixture.sql(&format!("ALTER TABLE {}.{retained} RENAME TO {}", marker.schema, marker.table)).await;
                } else {
                    let restore = match fault {
                        "type" => format!("ALTER TABLE {} ALTER COLUMN version TYPE integer USING version::integer", marker.name()),
                        "nullable" => format!("ALTER TABLE {} ALTER COLUMN version SET NOT NULL", marker.name()),
                        "primary_key" => format!("ALTER TABLE {} ADD CONSTRAINT {key} PRIMARY KEY (singleton)", marker.name()),
                        "row_security" => format!("ALTER TABLE {} DISABLE ROW LEVEL SECURITY", marker.name()),
                        _ => unreachable!(),
                    };
                    fixture.sql(&restore).await;
                }
                let mut connection = within(fixture.runtime().acquire()).await
                    .unwrap_or_else(|_| panic!("restored marker observation connection unavailable"));
                let restored = within(verify_platform_storage_markers(fixture.config(), &mut connection, &original_storage)).await;
                drop(connection);
                let rows_unchanged = before.same_rows(&snapshot(fixture).await);
                let expected = if fault == "replacement" { Error::StorageMarkersChanged } else { Error::UnsupportedStorageMarker };
                (result, expected, blocked, fault_identity_proven, replacement_ok, restored, rows_unchanged)
            }).catch_unwind().await;
            admin.close(fixture).await;
            results.push(match outcome { Ok(result) => result, Err(panic) => std::panic::resume_unwind(panic) });
        }
        results
    })).await;
    assert_eq!(results.len(), 9);
    for (
        result,
        expected,
        blocked,
        fault_identity_proven,
        replacement_ok,
        restored,
        rows_unchanged,
    ) in results
    {
        assert!(
            blocked,
            "the exact backend must really wait for the held relation lock before drift"
        );
        assert!(
            fault_identity_proven,
            "NOT NULL/PK/type/RLS faults must retain their original relation OID"
        );
        assert_eq!(result, Err(expected));
        assert!(replacement_ok);
        assert_eq!(restored, Ok(()));
        assert!(
            rows_unchanged,
            "explicit DDL faults and read-only observations must not change stored rows"
        );
    }
}
