use super::*;

pub(in super::super) struct Pending<T> {
    job: Option<tokio::task::JoinHandle<T>>,
}

impl<T: Send + 'static> Pending<T> {
    pub(in super::super) fn start(future: impl Future<Output = T> + Send + 'static) -> Self {
        Self {
            job: Some(tokio::spawn(future)),
        }
    }

    pub(in super::super) async fn finish(mut self) -> T {
        let result = within(self.job.as_mut().unwrap()).await;
        self.job.take();
        result.unwrap_or_else(|_| panic!("owned stock task failed"))
    }

    pub(in super::super) async fn cancel(mut self) -> bool {
        self.job.as_ref().unwrap().abort();
        let result = within(self.job.as_mut().unwrap()).await;
        self.job.take();
        result.is_err_and(|error| error.is_cancelled())
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}

impl Fixture {
    pub(in super::super) async fn sql(&self, statement: &str) {
        within(query(statement).execute(self.pool()))
            .await
            .unwrap_or_else(|_| panic!("owned synthetic stock SQL setup failed"));
    }

    pub(in super::super) async fn fault(
        &self,
        table: &str,
        timing: &str,
        body: &str,
        deferred: bool,
    ) {
        assert!(matches!(table, "users" | "sessions"));
        self.sql("CREATE SEQUENCE stock_fault_calls").await;
        self.sql(&format!(
            "CREATE FUNCTION stock_fault() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN {body} END $$"
        ))
        .await;
        let kind = if deferred { "CONSTRAINT " } else { "" };
        let defer = if deferred {
            "DEFERRABLE INITIALLY DEFERRED"
        } else {
            ""
        };
        self.sql(&format!(
            "CREATE {kind}TRIGGER stock_fault {timing} ON {table} {defer}
            FOR EACH ROW EXECUTE FUNCTION stock_fault()"
        ))
        .await;
    }

    pub(in super::super) fn hit_sql(&self) -> String {
        format!("PERFORM nextval('{}.stock_fault_calls');", self.schema)
    }

    pub(in super::super) async fn calls(&self) -> i64 {
        within(query("SELECT CASE WHEN is_called THEN last_value ELSE 0 END AS calls FROM stock_fault_calls")
            .fetch_one(self.pool())).await
            .unwrap_or_else(|_| panic!("stock fault evidence unavailable"))
            .get("calls")
    }

    pub(in super::super) async fn shadow(&mut self) -> String {
        let schema = format!("stock_shadow_{}", Uuid::new_v4().simple());
        self.namespaces.push(schema.clone());
        within(query(&format!("CREATE SCHEMA {schema}")).execute(self.admin()))
            .await
            .unwrap_or_else(|_| panic!("owned stock shadow setup failed"));
        within(
            query(&format!(
                "CREATE TABLE {schema}.sessions (token TEXT PRIMARY KEY)"
            ))
            .execute(self.admin()),
        )
        .await
        .unwrap_or_else(|_| panic!("owned stock shadow relation setup failed"));
        schema
    }

    pub(in super::super) async fn independent(
        &mut self,
        suffix: &str,
        rr: bool,
    ) -> (DurableAuthSource, PgPool) {
        let schema = self.schema.clone();
        let application = format!("{}_{}", self.schema, suffix);
        let isolation = if rr {
            "repeatable read"
        } else {
            "read committed"
        };
        let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
        let pool = within(
            PgPoolOptions::new()
                .max_connections(1)
                .after_connect(move |connection, _| {
                    let schema = schema.clone();
                    let application = application.clone();
                    Box::pin(async move {
                        query(
                            "SELECT set_config('search_path', $1, false),
                    set_config('application_name', $2, false),
                    set_config('default_transaction_isolation', $3, false)",
                        )
                        .bind(schema)
                        .bind(application)
                        .bind(isolation)
                        .execute(connection)
                        .await?;
                        Ok(())
                    })
                })
                .connect(&url),
        )
        .await
        .unwrap_or_else(|_| panic!("independent stock Source pool unavailable"));
        self.pools.push(pool.clone());
        let mut source = DurableAuthSource::new(pool.clone(), self.authority);
        let clock = self.clock.clone();
        source.otp_clock = Some(Arc::new(move || {
            chrono::DateTime::from_timestamp(clock.load(Ordering::SeqCst), 0).unwrap()
        }));
        (source, pool)
    }

    pub(in super::super) async fn blocked_writer(&self, suffix: &str, blocker: i32) -> i32 {
        let application = format!("{}_{}", self.schema, suffix);
        within(async {
            loop {
                // A queued writer may wait on another writer's tuple lock. Verify a bounded
                // actual blocker path to our exact holder, not an arbitrary wait predicate.
                let row = query(
                    "WITH RECURSIVE blockers(waiter, pid, seen, depth) AS (
                    SELECT a.pid, a.pid, ARRAY[a.pid], 0 FROM pg_stat_activity a
                    WHERE a.datname=current_database() AND a.application_name=$1
                    AND a.query LIKE '%collaboration_auth_source_schema%FOR UPDATE%'
                    UNION ALL
                    SELECT b.waiter, p.pid, b.seen || p.pid, b.depth + 1
                    FROM blockers b CROSS JOIN LATERAL unnest(pg_blocking_pids(b.pid)) p(pid)
                    WHERE b.depth < 8 AND NOT (p.pid = ANY(b.seen))
                    ) SELECT waiter AS pid FROM blockers WHERE pid=$2 AND depth>0 LIMIT 1",
                )
                .bind(&application)
                .bind(blocker)
                .fetch_optional(self.admin())
                .await
                .unwrap_or_else(|_| panic!("actual writer wait observation failed"));
                if let Some(row) = row {
                    break row.get("pid");
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
    }

    pub(in super::super) async fn blocked_trigger(&self, blocker: i32) -> i32 {
        within(async {
            loop {
                let row = query(
                    "SELECT pid FROM pg_stat_activity WHERE datname=current_database()
                    AND application_name=$1 AND $2=ANY(pg_blocking_pids(pid)) LIMIT 1",
                )
                .bind(&self.schema)
                .bind(blocker)
                .fetch_optional(self.admin())
                .await
                .unwrap_or_else(|_| panic!("actual trigger wait observation failed"));
                if let Some(row) = row {
                    break row.get("pid");
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
    }

    pub(in super::super) async fn drain(&self) {
        let mut tx = within(self.pool().begin()).await.unwrap();
        within(query("SET LOCAL lock_timeout = '5s'").execute(&mut *tx))
            .await
            .unwrap();
        within(query("LOCK TABLE users, sessions IN ACCESS EXCLUSIVE MODE").execute(&mut *tx))
            .await
            .unwrap_or_else(|_| panic!("actual pending stock transaction did not drain"));
        within(tx.rollback()).await.unwrap();
    }
}
