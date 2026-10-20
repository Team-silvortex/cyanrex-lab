//! Real queued password work and disposable SQL, not a timing/sleep approximation of the gap.
use super::*;
use crate::services::auth_service::durable_source::{DurableAuthError, DurableLogin};
use sqlx_core::query::query;
use std::{sync::mpsc, time::Duration};
use tokio::task::JoinHandle;

mod config {
    use crate::models::collaboration::WorkspaceRef;
    use uuid::Uuid;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}

#[allow(dead_code)]
#[path = "../../../../tests/platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "../../../../tests/platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[allow(dead_code)]
#[path = "../../../../tests/platform_login_connection/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../../tests/platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "../../../../tests/platform_storage_connection/support.rs"]
mod storage_helpers;
use catalog_fixture::{within, Fixture};
use fixture::{otp, password, session_count, snapshot, username, watermark, Prepared};

struct BlockingRelease(Option<mpsc::Sender<()>>);
impl Drop for BlockingRelease {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

fn isolated_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap()
}

fn capacity() -> (usize, usize) {
    let gate = shared_gate();
    (
        gate.workers.available_permits(),
        gate.total.available_permits(),
    )
}

async fn occupy_executor() -> (BlockingRelease, JoinHandle<bool>) {
    let (release, wait) = mpsc::channel();
    let (entered, started) = tokio::sync::oneshot::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _ = entered.send(());
        wait.recv_timeout(Duration::from_secs(10)).is_ok()
    });
    within(started).await.unwrap();
    (BlockingRelease(Some(release)), blocker)
}

async fn dispatched(
    job: &JoinHandle<std::result::Result<DurableLogin, DurableAuthError>>,
    baseline: (usize, usize),
) -> bool {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if job.is_finished() {
                return false;
            }
            let now = capacity();
            if (now.0 + 1, now.1 + 1) == baseline {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

async fn recovered(baseline: (usize, usize)) -> bool {
    tokio::time::timeout(Duration::from_secs(5), async {
        while capacity() != baseline {
            tokio::task::yield_now().await;
        }
    })
    .await
    .is_ok()
}

async fn source_unlocked(fixture: &Fixture) -> bool {
    let mut tx = within(fixture.owner().begin()).await.unwrap();
    let row = within(
        query(&format!(
            "SELECT singleton FROM {}.collaboration_auth_source_schema FOR UPDATE NOWAIT",
            fixture.source
        ))
        .fetch_one(&mut *tx),
    )
    .await
    .is_ok();
    let relations = if row {
        within(
            query(&format!(
                "LOCK TABLE {}.users, {}.sessions, {}.collaboration_auth_source_schema,
            {}.collaboration_identity_schema, {}.collaboration_access_schema,
            {}.collaboration_task_schema, {}.collaboration_artifact_schema
            IN ACCESS EXCLUSIVE MODE NOWAIT",
                fixture.source,
                fixture.source,
                fixture.source,
                fixture.source,
                fixture.source,
                fixture.task,
                fixture.artifacts
            ))
            .execute(&mut *tx),
        )
        .await
        .is_ok()
    } else {
        false
    };
    let rolled_back = within(tx.rollback()).await.is_ok();
    row && relations && rolled_back
}

#[test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
fn postgres_platform_login_gap_cancellation_keeps_real_password_job_capacity_without_source_resources(
) {
    isolated_runtime().block_on(async {
        let (
            admitted,
            no_connection,
            unlocked,
            cancelled,
            retained,
            released,
            drained,
            unchanged,
            empty,
            cleaned,
        ) = Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let connector = Arc::new(prepared.connector().await);
                let before = snapshot(f).await;
                let baseline = capacity();
                let (release, blocker) = occupy_executor().await;
                let owned = connector.clone();
                let code = otp();
                let pending =
                    tokio::spawn(async move { owned.login(&username(), password(), &code).await });
                // Both real password permits are held by a job queued behind the occupied executor.
                let admitted = dispatched(&pending, baseline).await;
                let no_connection = storage_helpers::no_connections(f).await;
                let unlocked = source_unlocked(f).await;
                pending.abort();
                let cancelled = within(pending)
                    .await
                    .is_err_and(|error| error.is_cancelled());
                let now = capacity();
                let retained = (now.0 + 1, now.1 + 1) == baseline;
                drop(release);
                let released = within(blocker).await.unwrap();
                let drained = recovered(baseline).await;
                let unchanged = before == snapshot(f).await;
                let empty = session_count(f).await == 0 && watermark(f).await == -1;
                drop(connector);
                let cleaned = prepared.finish();
                (
                    admitted,
                    no_connection,
                    unlocked,
                    cancelled,
                    retained,
                    released,
                    drained,
                    unchanged,
                    empty,
                    cleaned,
                )
            })
        })
        .await;
        assert!(
            admitted,
            "the real password job was not admitted into the occupied executor"
        );
        assert!(
            no_connection && unlocked,
            "the password gap retained a source connection or lock"
        );
        assert!(cancelled && retained && released && drained);
        assert!(unchanged && empty && cleaned);
    });
}

#[derive(Clone, Copy)]
enum Drift {
    Root,
    Marker,
    Namespace,
}

#[test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
fn postgres_platform_login_gap_rejects_original_root_marker_and_namespace_drift_without_otp_or_session_writes(
) {
    isolated_runtime().block_on(async {
        for drift in [Drift::Root, Drift::Marker, Drift::Namespace] {
            let (admitted, unlocked, changed, released, result, drained, unchanged, empty, restored, healthy, no_connection, cleaned) = Fixture::checked(|f| Box::pin(async move {
                let auxiliary = if matches!(drift, Drift::Namespace) { Some(f.additional_role().await) } else { None };
                let owner = expectation_helpers::quote_identifier(&f.owner_name().await);
                let prepared = Prepared::new(f).await;
                let connector = Arc::new(prepared.connector().await);
                let before = snapshot(f).await;
                let baseline = capacity();
                let (release, blocker) = occupy_executor().await;
                let owned = connector.clone();
                let code = otp();
                let pending = tokio::spawn(async move { owned.login(&username(), password(), &code).await });
                let admitted = dispatched(&pending, baseline).await;
                let unlocked = storage_helpers::no_connections(f).await && source_unlocked(f).await;
                let changed = match drift {
                    Drift::Root => prepared.root().swap(),
                    Drift::Marker => {
                        // Replace a non-Source marker so the old Source-only pin cannot mask
                        // a missing retained-target check on the second connection.
                        f.sql(&format!("ALTER TABLE {}.collaboration_task_schema RENAME TO gap_saved_task_marker", f.task)).await;
                        f.sql(&format!("CREATE TABLE {}.collaboration_task_schema (LIKE {}.gap_saved_task_marker INCLUDING ALL)", f.task, f.task)).await;
                        f.sql(&format!("INSERT INTO {}.collaboration_task_schema SELECT * FROM {}.gap_saved_task_marker", f.task, f.task)).await;
                        f.sql(&format!("GRANT SELECT ON {}.collaboration_task_schema TO {}", f.task, f.role)).await;
                        true
                    }
                    Drift::Namespace => {
                        f.sql(&format!("ALTER SCHEMA {} OWNER TO {}", f.artifacts, auxiliary.as_ref().unwrap())).await;
                        true
                    }
                };
                let drifted = snapshot(f).await;
                drop(release);
                let released = within(blocker).await.unwrap();
                let result = within(pending).await.unwrap().map(|_| ());
                let drained = recovered(baseline).await;
                let unchanged = drifted == snapshot(f).await;
                let empty = session_count(f).await == 0 && watermark(f).await == -1;
                let restored = match drift {
                    Drift::Root => prepared.root().restore(),
                    Drift::Marker => {
                        f.sql(&format!("DROP TABLE {}.collaboration_task_schema", f.task)).await;
                        f.sql(&format!("ALTER TABLE {}.gap_saved_task_marker RENAME TO collaboration_task_schema", f.task)).await;
                        true
                    }
                    Drift::Namespace => {
                        f.sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", f.artifacts)).await;
                        true
                    }
                } && before == snapshot(f).await;
                // Restore only the original identities, then explicitly retry as a new call.
                // The failed call has neither consumed its OTP nor adopted a new baseline.
                let healthy = connector.login(&username(), password(), &otp()).await.is_ok();
                drop(connector);
                let no_connection = storage_helpers::no_connections(f).await;
                let cleaned = prepared.finish();
                (admitted, unlocked, changed, released, result, drained, unchanged, empty, restored, healthy, no_connection, cleaned)
            })).await;
            assert!(admitted && unlocked && changed && released && drained);
            assert_eq!(result, Err(DurableAuthError::StorageUnavailable));
            assert!(unchanged && empty && restored && healthy && no_connection && cleaned);
        }
    });
}

#[test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
fn postgres_platform_login_gap_cannot_adopt_an_account_created_after_initial_absence() {
    isolated_runtime().block_on(async {
        let (admitted, unlocked, released, result, drained, unchanged, empty, healthy, no_connection, cleaned) = Fixture::checked(|f| Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let connector = Arc::new(prepared.connector().await);
            let baseline = capacity();
            let (release, blocker) = occupy_executor().await;
            let owned = connector.clone();
            let missing: crate::models::collaboration::LegacyUsername = "gap-created-account".parse().unwrap();
            let name = missing.clone();
            let code = otp();
            let pending = tokio::spawn(async move { owned.login(&name, "cyanrex-public-missing-account-v1", &code).await });
            let admitted = dispatched(&pending, baseline).await;
            let unlocked = storage_helpers::no_connections(f).await && source_unlocked(f).await;
            // Both dummy and later real credentials genuinely match the same public password.
            // A wrong-password test could hide an unsafe second lookup after the gap.
            within(query(&format!("INSERT INTO {}.users(username,account_id,password_salt,password_hash,totp_secret)
                VALUES ($1,$2,'cyanrex-public-missing-account',$3,'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ')", f.source))
                .bind(missing.as_str()).bind(uuid::Uuid::new_v4())
                .bind(password_profile::MISSING_ACCOUNT_HASH).execute(f.owner())).await.unwrap();
            let created = snapshot(f).await;
            drop(release);
            let released = within(blocker).await.unwrap();
            let result = within(pending).await.unwrap().map(|_| ());
            let drained = recovered(baseline).await;
            let unchanged = created == snapshot(f).await;
            let empty = session_count(f).await == 0;
            let healthy = connector.login(&missing, "cyanrex-public-missing-account-v1", &otp()).await.is_ok();
            drop(connector);
            let no_connection = storage_helpers::no_connections(f).await;
            let cleaned = prepared.finish();
            (admitted, unlocked, released, result, drained, unchanged, empty, healthy, no_connection, cleaned)
        })).await;
        assert!(admitted && unlocked && released && drained);
        assert_eq!(result, Err(DurableAuthError::InvalidCredentials));
        assert!(unchanged && empty && healthy && no_connection && cleaned);
    });
}
