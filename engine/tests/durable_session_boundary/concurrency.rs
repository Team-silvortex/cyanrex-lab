use super::*;
use std::time::Duration;

type Tx<'a> = sqlx_core::transaction::Transaction<'a, sqlx_postgres::Postgres>;
async fn backend(tx: &mut Tx<'_>) -> i32 {
    query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut **tx)
        .await
        .unwrap()
        .get("pid")
}
async fn blocked_by(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(row) = query(
                "SELECT pid FROM pg_stat_activity WHERE datname = current_database()
                AND $1 = ANY(pg_blocking_pids(pid)) ORDER BY pid LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(&f.admin)
            .await
            .unwrap()
            {
                return row.get("pid");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Session operation did not reach its exact synthetic blocker")
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_validate_rechecks_expiry_after_source_lock_wait() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(token_hash(&login.token)).execute(&f.pool).await.unwrap();
    let mut holder = f.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let blocker = backend(&mut holder).await;
    let source = f.reopen().await;
    let token = login.token.clone();
    let pending = tokio::spawn(async move { source.validate_session(&token).await });
    blocked_by(&f, blocker).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!("SELECT expires_at <= clock_timestamp() AS expired FROM {}.sessions WHERE token = $1", f.schema))
                .bind(token_hash(&login.token)).fetch_one(&f.admin).await.unwrap().get("expired");
            if expired { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    assert!(!pending.is_finished());
    holder.rollback().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), None);
    assert_eq!(
        f.count("sessions").await,
        1,
        "a read must not delete expired sessions"
    );
    f.drain().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_validate_waits_for_exact_logout_commit() {
    let f = Fixture::ready().await;
    let (_, login) = f.login().await;
    fault(
        &f,
        "BEFORE DELETE",
        "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 74301); RETURN OLD;",
    )
    .await;
    let mut holder = f.pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 74301)")
        .execute(&mut *holder)
        .await
        .unwrap();
    let blocker = backend(&mut holder).await;
    let source = f.source.clone();
    let token = login.token.clone();
    let logout = tokio::spawn(async move { source.logout(&token).await });
    let writer = blocked_by(&f, blocker).await;
    let source = f.reopen().await;
    let token = login.token.clone();
    let read = tokio::spawn(async move { source.validate_session(&token).await });
    blocked_by(&f, writer).await;
    assert!(!read.is_finished());
    holder.rollback().await.unwrap();
    logout.await.unwrap().unwrap();
    assert_eq!(read.await.unwrap().unwrap(), None);
    assert_eq!(calls(&f).await, 1);
    assert_eq!(f.count("sessions").await, 0);
    f.cleanup().await;
}

struct PasswordGate(Option<std::sync::mpsc::Sender<()>>);
impl Drop for PasswordGate {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_boundary_login_retains_its_first_namespace_pin_across_password_work() {
    // A private executor gives the Argon2 worker a deterministic queue boundary, not a race sleep.
    tokio::task::spawn_blocking(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let f = Fixture::ready().await;
            let created = f.source.register(&username(), PASSWORD).await.unwrap();
            let application = format!("boundary_hash_{}", Uuid::new_v4().simple());
            let source_pool = scoped_pool(&f.schema, &application).await;
            let source = DurableAuthSource::new(source_pool.clone(), authority());
            let (entered_sender, entered) = std::sync::mpsc::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let gate = PasswordGate(Some(release));
            let worker = tokio::task::spawn_blocking(move || {
                entered_sender.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(10))
                    .expect("password-work gate was not released");
            });
            entered.recv_timeout(Duration::from_secs(2)).unwrap();
            let code = otp(&created.bootstrap.secret);
            let pending =
                tokio::spawn(async move { source.login(&username(), PASSWORD, &code).await });
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let committed: bool = query(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                        WHERE datname = current_database() AND application_name = $1
                        AND state = 'idle' AND query = 'COMMIT') AS committed",
                    )
                    .bind(&application)
                    .fetch_one(&f.admin)
                    .await
                    .unwrap()
                    .get("committed");
                    if committed {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("login never released its first authentication read transaction");
            assert!(!pending.is_finished());
            let old_schema = format!("saved_source_{}", Uuid::new_v4().simple());
            query(&format!("ALTER SCHEMA {} RENAME TO {old_schema}", f.schema))
                .execute(&f.admin)
                .await
                .unwrap();
            query(&format!("CREATE SCHEMA {}", f.schema))
                .execute(&f.admin)
                .await
                .unwrap();
            // Explicit trusted fixture preparation; same source/account/credentials, new namespace OID.
            f.source.install_empty_schema().await.unwrap();
            query(&format!(
                "INSERT INTO {}.users SELECT * FROM {old_schema}.users",
                f.schema
            ))
            .execute(&f.admin)
            .await
            .unwrap();
            drop(gate);
            worker.await.unwrap();
            let result = pending.await.unwrap();
            assert!(
                result.is_err(),
                "login discarded its first pin and adopted an identical replacement namespace"
            );
            assert_eq!(f.count("sessions").await, 0);
            let old_sessions: i64 =
                query(&format!("SELECT count(*) AS n FROM {old_schema}.sessions"))
                    .fetch_one(&f.admin)
                    .await
                    .unwrap()
                    .get("n");
            assert_eq!(old_sessions, 0);
            source_pool.close().await;
            query(&format!("DROP SCHEMA {old_schema} CASCADE"))
                .execute(&f.admin)
                .await
                .unwrap();
            f.cleanup().await;
        });
    })
    .await
    .unwrap();
}
