use super::{fixture::Restricted, http_fixture::*};
use axum::http::StatusCode;
use serde_json::json;
use sqlx_core::{query::query, row::Row};

const SLOT: &str = "cyanrex_session_source_relations_v1";

fn trap_error(error: sqlx_core::error::Error) -> bool {
    error
        .as_database_error()
        .and_then(|database| database.code())
        .is_some_and(|code| code == "P0001")
}

async fn calls(r: &Restricted<'_>) -> (bool, i64) {
    let row = within(
        query("SELECT is_called, last_value FROM permission_trap_calls").fetch_one(r.owner.pool()),
    )
    .await
    .unwrap_or_else(|_| panic!("independent immutable trap counter unavailable"));
    (row.get("is_called"), row.get("last_value"))
}

async fn positive_control(r: &Restricted<'_>, statement: &str) -> bool {
    // The administrator independently exercises the unsafe relation under the actual runtime role.
    // A superuser SELECT would bypass RLS and would not be a meaningful positive control.
    let mut tx = within(r.owner.pool().begin())
        .await
        .unwrap_or_else(|_| panic!("positive-control transaction unavailable"));
    within(query(&format!("SET LOCAL ROLE {}", r.role)).execute(&mut *tx))
        .await
        .unwrap_or_else(|_| panic!("positive-control runtime role unavailable"));
    let trapped = within(query(statement).fetch_all(&mut *tx))
        .await
        .err()
        .is_some_and(trap_error);
    within(tx.rollback())
        .await
        .unwrap_or_else(|_| panic!("positive-control rollback failed"));
    trapped
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_permissions_parse_only_rejects_view_rls_and_preserves_a_colliding_plan() {
    for mode in ["view", "rls", "collision"] {
        let (before, after, reply, restored, positive, no_execution, preserved, clean_slots, acl) =
            Fixture::checked(|f| Box::pin(async move {
                Restricted::checked(f, |r| Box::pin(async move {
                    r.admin_sql("CREATE SEQUENCE permission_trap_calls").await;
                    r.admin_sql(&format!("CREATE FUNCTION permission_trap() RETURNS integer
                        LANGUAGE plpgsql IMMUTABLE SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN
                        PERFORM nextval('{}.permission_trap_calls');
                        RAISE EXCEPTION 'synthetic immutable fixture trap'; END $$", r.owner.schema)).await;
                    let before = r.owner.snapshot().await;
                    let (reply, positive, no_execution, preserved) = match mode {
                        "view" => {
                            r.admin_sql("ALTER TABLE users RENAME TO session_permissions_saved_users").await;
                            r.admin_sql("CREATE VIEW users AS SELECT * FROM session_permissions_saved_users
                                WHERE permission_trap()=1").await;
                            r.admin_sql(&format!("GRANT SELECT (username, account_id) ON users TO {}", r.role)).await;
                            let positive = positive_control(r, "SELECT username FROM users LIMIT 1").await;
                            let counter = calls(r).await;
                            let reply = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                            let no_execution = calls(r).await == counter && counter == (true, 1);
                            r.admin_sql("DROP VIEW users").await;
                            r.admin_sql("ALTER TABLE session_permissions_saved_users RENAME TO users").await;
                            (reply, positive, no_execution, true)
                        }
                        "rls" => {
                            r.admin_sql("ALTER TABLE users ENABLE ROW LEVEL SECURITY").await;
                            r.admin_sql("ALTER TABLE users FORCE ROW LEVEL SECURITY").await;
                            r.admin_sql("CREATE POLICY permission_immutable_trap ON users FOR SELECT
                                USING (permission_trap()=1)").await;
                            let positive = positive_control(r, "SELECT username FROM users LIMIT 1").await;
                            let counter = calls(r).await;
                            let reply = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                            let no_execution = calls(r).await == counter && counter == (true, 1);
                            r.admin_sql("DROP POLICY permission_immutable_trap ON users").await;
                            r.admin_sql("ALTER TABLE users NO FORCE ROW LEVEL SECURITY").await;
                            r.admin_sql("ALTER TABLE users DISABLE ROW LEVEL SECURITY").await;
                            (reply, positive, no_execution, true)
                        }
                        "collision" => {
                            let positive = positive_control(r, "SELECT permission_trap()").await;
                            // Hold the other connection so the Source must use this exact session.
                            let held = within(r.pool().acquire()).await
                                .unwrap_or_else(|_| panic!("collision companion connection unavailable"));
                            let mut connection = within(r.pool().acquire()).await
                                .unwrap_or_else(|_| panic!("collision candidate connection unavailable"));
                            let original = format!("PREPARE {SLOT} AS SELECT permission_trap()");
                            within(query(&original).execute(&mut *connection)).await
                                .unwrap_or_else(|_| panic!("owned colliding plan setup failed"));
                            drop(connection);
                            let counter = calls(r).await;
                            let reply = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                            let no_execution = calls(r).await == counter && counter == (true, 1);
                            let mut connection = within(r.pool().acquire()).await
                                .unwrap_or_else(|_| panic!("original colliding connection unavailable"));
                            let preserved: bool = within(query("SELECT EXISTS (SELECT 1 FROM
                                pg_catalog.pg_prepared_statements WHERE name=$1 AND statement=$2) AS preserved")
                                .bind(SLOT).bind(&original).fetch_one(&mut *connection)).await
                                .unwrap_or_else(|_| panic!("original plan identity observation failed")).get("preserved");
                            let executable = within(query(&format!("EXECUTE {SLOT}"))
                                .fetch_all(&mut *connection)).await.err().is_some_and(trap_error);
                            let executed_once = calls(r).await == (true, 2);
                            within(query(&format!("DEALLOCATE {SLOT}")).execute(&mut *connection)).await
                                .unwrap_or_else(|_| panic!("owned collision plan cleanup failed"));
                            drop(connection);
                            drop(held);
                            (reply, positive, no_execution, preserved && executable && executed_once)
                        }
                        _ => unreachable!(),
                    };
                    let restored = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                    let clean_slots = r.no_owned_prepared_slots().await;
                    let acl = r.minimal_acl().await;
                    let after = r.owner.snapshot().await;
                    (before, after, reply, restored, positive, no_execution, preserved, clean_slots, acl)
                })).await
            })).await;
        denied(
            &reply,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
        assert!(
            positive && no_execution,
            "unsafe immutable relation was not independently callable or was executed by pinning"
        );
        assert!(
            preserved,
            "a fixed-name collision executed, changed or removed the original plan"
        );
        successful(
            &restored,
            json!({"schema_version":1,"authenticated":true}),
            false,
        );
        assert!(
            clean_slots && acl,
            "restoration retained a prepared slot or extra grants"
        );
        assert!(
            unchanged(&before, &after),
            "shape or collision rejection changed Source data or pins"
        );
    }
    let removed = Fixture::checked(|f| Box::pin(Restricted::cleanup_after_partial_view(f))).await;
    assert!(removed, "partial view setup retained its synthetic role");
}
